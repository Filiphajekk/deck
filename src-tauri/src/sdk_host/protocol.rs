//! `deckplugin://` URI scheme: serves plugin files (Property Inspector pages, HTML plugins)
//! and injects the `connectElgatoStreamDeckSocket(...)` call the SDK expects.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use tauri::http::{Request, Response, StatusCode};

pub const SCHEME: &str = "deckplugin";

fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn dec(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// URL of a plugin page. `connect` holds the arguments for connectElgatoStreamDeckSocket.
pub fn page_url(plugin: &str, rel_path: &str, connect: &Value) -> String {
    let path: Vec<String> = rel_path
        .replace('\\', "/")
        .split('/')
        .filter(|s| !s.is_empty())
        .map(enc)
        .collect();
    format!(
        "http://{SCHEME}.localhost/{}/{}?dl={}",
        enc(plugin),
        path.join("/"),
        enc(&connect.to_string())
    )
}

fn mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" | "cjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

/// Resolves `/<plugin>/<file...>` to a file inside that plugin's folder (no escaping out).
pub fn resolve(plugins_dir: &Path, url_path: &str) -> Option<PathBuf> {
    let mut parts = url_path.trim_start_matches('/').splitn(2, '/');
    let plugin = dec(parts.next()?);
    let rel = dec(parts.next().unwrap_or(""));
    if plugin.is_empty() || plugin.contains(['/', '\\']) || plugin.contains("..") {
        return None;
    }
    let root = plugins_dir.join(format!("{plugin}.sdPlugin"));
    let mut full = root.clone();
    for c in Path::new(&rel.replace('/', "\\")).components() {
        match c {
            Component::Normal(s) => full.push(s),
            Component::CurDir => {}
            _ => return None,
        }
    }
    full.starts_with(&root).then_some(full)
}

fn connect_script(connect_json: &str) -> String {
    // The JSON is passed URI-encoded so it cannot break out of the string literal.
    format!(
        r#"<script>(function(){{var p=JSON.parse(decodeURIComponent("{}"));var tries=0;function go(){{if(typeof window.connectElgatoStreamDeckSocket==="function"){{window.connectElgatoStreamDeckSocket(p.port,p.uuid,p.event,p.info,p.actionInfo);}}else if(tries++<200){{setTimeout(go,50);}}}}if(document.readyState==="complete")go();else window.addEventListener("load",go);}})();</script>"#,
        enc(connect_json)
    )
}

/// Inserts the connect script before `</body>` (or appends it).
pub fn inject(html: &str, connect_json: &str) -> String {
    let script = connect_script(connect_json);
    match html.to_lowercase().rfind("</body>") {
        Some(i) => format!("{}{}{}", &html[..i], script, &html[i..]),
        None => format!("{html}{script}"),
    }
}

pub fn handle(request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let not_found = || {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new())
            .unwrap()
    };
    let uri = request.uri();
    let Some(path) = resolve(&crate::paths::plugins_dir(), uri.path()) else {
        return not_found();
    };
    let Ok(mut body) = std::fs::read(&path) else {
        return not_found();
    };
    let content_type = mime(&path);
    if content_type.starts_with("text/html") {
        if let Some(dl) = uri
            .query()
            .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("dl=")))
        {
            let html = String::from_utf8_lossy(&body).into_owned();
            body = inject(&html, &dec(dl)).into_bytes();
        }
    }
    Response::builder()
        .header("Content-Type", content_type)
        .header("Access-Control-Allow-Origin", "*")
        .body(body)
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_inside_plugin_only() {
        let root = Path::new(r"C:\data\plugins");
        assert_eq!(
            resolve(root, "/com.a.b/ui/pi.html"),
            Some(PathBuf::from(
                r"C:\data\plugins\com.a.b.sdPlugin\ui\pi.html"
            ))
        );
        assert_eq!(resolve(root, "/com.a.b/../../secret.txt"), None);
        assert_eq!(resolve(root, "/com.a.b/%2E%2E/x"), None);
        assert_eq!(resolve(root, "/../x/y"), None);
    }

    #[test]
    fn url_roundtrip_and_injection() {
        let url = page_url(
            "com.a.b",
            r"ui\pi index.html",
            &json!({"port": 1, "uuid": "x"}),
        );
        assert!(url.starts_with("http://deckplugin.localhost/com.a.b/ui/pi%20index.html?dl="));
        let dl = url.split("dl=").nth(1).unwrap();
        assert_eq!(dec(dl), r#"{"port":1,"uuid":"x"}"#);
        let html = inject("<html><body><p>x</p></body></html>", r#"{"port":1}"#);
        assert!(html.contains("connectElgatoStreamDeckSocket"));
        assert!(html.find("<script>").unwrap() < html.find("</body>").unwrap());
    }
}
