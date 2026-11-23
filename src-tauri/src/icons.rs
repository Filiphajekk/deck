//! Icons: extraction from executables (cached on disk) and scaling user images to 144×144.

use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::Path;

use anyhow::{bail, Context};
use base64::Engine;
use image::{imageops::FilterType, DynamicImage, ImageFormat, RgbaImage};
use windows::core::HSTRING;
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GetIconInfo, PrivateExtractIconsW, HICON, ICONINFO,
};

use crate::paths;

/// Maximum icon edge in pixels (SPEC 8).
pub const ICON_SIZE: u32 = 144;

fn to_data_url(png: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

fn encode_png(img: &DynamicImage) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)?;
    Ok(out)
}

/// Scales an image down so that it fits into 144×144 (never up).
pub fn fit(img: DynamicImage) -> DynamicImage {
    if img.width() <= ICON_SIZE && img.height() <= ICON_SIZE {
        img
    } else {
        img.resize(ICON_SIZE, ICON_SIZE, FilterType::Lanczos3)
    }
}

/// Loads a user image (PNG/JPG/GIF/WebP/SVG) and returns a data URL no larger than 144×144.
/// SVG stays vector; GIF/PNG/JPG are decoded (first frame) and re-encoded as PNG.
pub fn load_image_data_url(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    if ext == "svg" {
        if bytes.len() > 512 * 1024 {
            bail!("SVG is too large");
        }
        return Ok(format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        ));
    }
    let img = image::load_from_memory(&bytes).context("unsupported image")?;
    Ok(to_data_url(&encode_png(&fit(img))?))
}

/// Icon of an executable (or any file / shortcut) as a PNG data URL, cached on disk by
/// path + modification time.
pub fn file_icon_data_url(path: &str) -> anyhow::Result<String> {
    let meta = std::fs::metadata(path).with_context(|| format!("{path} not found"))?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.to_lowercase().hash(&mut h);
    meta.modified().ok().hash(&mut h);
    let cache = paths::icon_cache_dir().join(format!("{:016x}.png", h.finish()));
    if let Ok(png) = std::fs::read(&cache) {
        return Ok(to_data_url(&png));
    }
    let img = extract_icon(path)?;
    let png = encode_png(&fit(DynamicImage::ImageRgba8(img)))?;
    let _ = crate::fsutil::write_atomic(&cache, &png);
    Ok(to_data_url(&png))
}

fn extract_icon(path: &str) -> anyhow::Result<RgbaImage> {
    let wide = HSTRING::from(path);
    // Prefer the 256 px icon from the resource; fall back to the shell's large icon.
    let mut buf = [0u16; 260];
    let src: Vec<u16> = path.encode_utf16().collect();
    let n = src.len().min(259);
    buf[..n].copy_from_slice(&src[..n]);
    let mut icons = [HICON::default()];
    let mut ids = [0u32];
    let count = unsafe {
        PrivateExtractIconsW(
            &buf,
            0,
            256,
            256,
            Some(&mut icons),
            Some(ids.as_mut_ptr()),
            0,
        )
    };
    let hicon = if count > 0 && count != u32::MAX && !icons[0].is_invalid() {
        icons[0]
    } else {
        let mut info = SHFILEINFOW::default();
        let ok = unsafe {
            SHGetFileInfoW(
                &wide,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            )
        };
        if ok == 0 || info.hIcon.is_invalid() {
            bail!("no icon in {path}");
        }
        info.hIcon
    };
    let result = hicon_to_rgba(hicon);
    unsafe {
        let _ = DestroyIcon(hicon);
    }
    result
}

fn hicon_to_rgba(hicon: HICON) -> anyhow::Result<RgbaImage> {
    unsafe {
        let mut ii = ICONINFO::default();
        GetIconInfo(hicon, &mut ii)?;
        let color = ii.hbmColor;
        let mask = ii.hbmMask;
        let cleanup = || {
            if !color.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(color.0));
            }
            if !mask.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(mask.0));
            }
        };
        if color.is_invalid() {
            cleanup();
            bail!("monochrome icons are not supported");
        }
        let mut bm = BITMAP::default();
        GetObjectW(
            HGDIOBJ(color.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut _),
        );
        let (w, h) = (bm.bmWidth, bm.bmHeight);
        if w <= 0 || h <= 0 {
            cleanup();
            bail!("empty icon");
        }
        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let dc = GetDC(None);
        let lines = GetDIBits(
            dc,
            color,
            0,
            h as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bi,
            DIB_RGB_COLORS,
        );
        // Mask bits (1 = transparent) for icons without an alpha channel.
        let mut mask_px = vec![0u8; (w * h * 4) as usize];
        let has_mask = !mask.is_invalid()
            && GetDIBits(
                dc,
                mask,
                0,
                h as u32,
                Some(mask_px.as_mut_ptr() as *mut _),
                &mut bi,
                DIB_RGB_COLORS,
            ) > 0;
        ReleaseDC(None, dc);
        cleanup();
        if lines == 0 {
            bail!("GetDIBits failed");
        }
        let has_alpha = pixels.chunks(4).any(|p| p[3] != 0);
        for (i, p) in pixels.chunks_mut(4).enumerate() {
            p.swap(0, 2); // BGRA -> RGBA
            if !has_alpha {
                let transparent = has_mask && mask_px[i * 4] != 0;
                p[3] = if transparent { 0 } else { 255 };
            }
        }
        RgbaImage::from_raw(w as u32, h as u32, pixels).context("bad icon buffer")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_scales_down_only() {
        let big = DynamicImage::new_rgba8(512, 256);
        let f = fit(big);
        assert_eq!((f.width(), f.height()), (144, 72));
        let small = DynamicImage::new_rgba8(32, 32);
        assert_eq!(fit(small).width(), 32);
    }

    #[test]
    fn extracts_explorer_icon() {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        let img = extract_icon(&format!(r"{windir}\explorer.exe")).unwrap();
        assert!(img.width() >= 32);
        assert!(img.pixels().any(|p| p.0[3] > 0));
    }
}
