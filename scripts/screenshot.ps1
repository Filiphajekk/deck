# Dev helper: captures the screen (or a region of it) into a PNG for UI checks.
# -X/-Y/-W/-H select a region in physical pixels; default is the whole virtual screen.
param([string]$Out = "screen.png", [double]$Scale = 0.5, [int]$X = 0, [int]$Y = 0, [int]$W = 0, [int]$H = 0)
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class Dpi { [DllImport("user32.dll")] public static extern IntPtr SetProcessDpiAwarenessContext(IntPtr v); }
"@
[Dpi]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
$b = [System.Windows.Forms.SystemInformation]::VirtualScreen
if ($W -eq 0) { $X = $b.Left; $Y = $b.Top; $W = $b.Width; $H = $b.Height }
$bmp = New-Object System.Drawing.Bitmap $W, $H
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($X, $Y, 0, 0, $bmp.Size)
$small = New-Object System.Drawing.Bitmap $bmp, ([int]($W * $Scale)), ([int]($H * $Scale))
$small.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
"virtual screen: $($b.Left),$($b.Top) $($b.Width)x$($b.Height)"
