# Dev helper: prints how many seconds the user has been idle (no keyboard/mouse input).
# Automated UI tests only inject input when the user is idle, so they never fight a real user.
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class Idle {
  [StructLayout(LayoutKind.Sequential)] public struct LII { public uint cbSize; public uint dwTime; }
  [DllImport("user32.dll")] public static extern bool GetLastInputInfo(ref LII p);
  public static uint Seconds() { var l = new LII(); l.cbSize = 8; GetLastInputInfo(ref l); return ((uint)Environment.TickCount - l.dwTime) / 1000; }
}
"@
[Idle]::Seconds()
