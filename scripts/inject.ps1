# Dev helper for manual/automated UI checks: injects keyboard and mouse input with SendInput.
# Steps (space separated):
#   down:<vk>  up:<vk>  tap:<vk>   (vk as hex like 0xA2 or decimal)
#   move:<x>,<y>                     absolute physical screen position (per-monitor DPI aware)
#   click / rclick                   left/right click at the current position
#   sleep:<ms>
# Example (Ctrl+Shift+Space tap): down:0xA2 down:0xA0 tap:0x20 up:0xA0 up:0xA2
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Steps)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Inj {
  [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Explicit)] public struct U { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }
  [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public U u; }
  [DllImport("user32.dll", SetLastError = true)] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
  [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint type);
  [DllImport("user32.dll")] public static extern IntPtr SetProcessDpiAwarenessContext(IntPtr v);
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
  static void Send(INPUT i) { uint n = SendInput(1, new INPUT[] { i }, Marshal.SizeOf(typeof(INPUT))); if (n == 0) Console.WriteLine("SendInput failed: " + Marshal.GetLastWin32Error() + " size=" + Marshal.SizeOf(typeof(INPUT))); }
  public static void Key(ushort vk, bool up) {
    var i = new INPUT(); i.type = 1; i.u.ki.wVk = vk; i.u.ki.wScan = (ushort)MapVirtualKey(vk, 0);
    uint f = up ? 2u : 0u;
    if (vk == 0x25 || vk == 0x26 || vk == 0x27 || vk == 0x28 || vk == 0x5B || vk == 0x5C || vk == 0x2E) f |= 1;
    i.u.ki.dwFlags = f; Send(i);
  }
  public static void Move(int x, int y) {
    int vx = GetSystemMetrics(76), vy = GetSystemMetrics(77), vw = GetSystemMetrics(78), vh = GetSystemMetrics(79);
    var i = new INPUT(); i.type = 0;
    i.u.mi.dx = (int)Math.Round((x - vx) * 65535.0 / (vw - 1));
    i.u.mi.dy = (int)Math.Round((y - vy) * 65535.0 / (vh - 1));
    i.u.mi.dwFlags = 0x0001 | 0x8000 | 0x4000; Send(i);
  }
  public static void Click(bool right) {
    var i = new INPUT(); i.type = 0; i.u.mi.dwFlags = right ? 0x0008u : 0x0002u; Send(i);
    i.u.mi.dwFlags = right ? 0x0010u : 0x0004u; Send(i);
  }
}
"@
[Inj]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null

function Parse-Vk([string]$s) { if ($s.StartsWith("0x")) { [Convert]::ToUInt16($s.Substring(2), 16) } else { [UInt16]$s } }

foreach ($step in $Steps) {
  $parts = $step.Split(":", 2)
  switch ($parts[0]) {
    "down" { [Inj]::Key((Parse-Vk $parts[1]), $false) }
    "up" { [Inj]::Key((Parse-Vk $parts[1]), $true) }
    "tap" { $vk = Parse-Vk $parts[1]; [Inj]::Key($vk, $false); Start-Sleep -Milliseconds 20; [Inj]::Key($vk, $true) }
    "move" { $xy = $parts[1].Split(","); [Inj]::Move([int]$xy[0], [int]$xy[1]) }
    "click" { [Inj]::Click($false) }
    "rclick" { [Inj]::Click($true) }
    "sleep" { Start-Sleep -Milliseconds ([int]$parts[1]) }
  }
  Start-Sleep -Milliseconds 15
}
