# Dev helper: captures the whole virtual screen into a PNG (used for manual/automated UI checks).
param([string]$Out = "screen.png", [double]$Scale = 0.5)
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System.Runtime.InteropServices;
public static class Dpi { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware(); }
"@
[Dpi]::SetProcessDPIAware() | Out-Null
$b = [System.Windows.Forms.SystemInformation]::VirtualScreen
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
$w = [int]($b.Width * $Scale); $h = [int]($b.Height * $Scale)
$small = New-Object System.Drawing.Bitmap $bmp, $w, $h
$small.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
"$($b.Left),$($b.Top) $($b.Width)x$($b.Height)"
