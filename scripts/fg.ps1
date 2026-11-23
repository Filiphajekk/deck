# Dev helper: prints the title and process of the foreground window.
Add-Type -MemberDefinition @'
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
'@ -Name U -Namespace W
$h = [W.U]::GetForegroundWindow(); $procId = 0; [W.U]::GetWindowThreadProcessId($h, [ref]$procId) | Out-Null
$sb = New-Object System.Text.StringBuilder 256; [W.U]::GetWindowText($h, $sb, 256) | Out-Null
"$((Get-Process -Id $procId).ProcessName)|$($sb.ToString())"
