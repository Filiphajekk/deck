# Dev helper: stops the dev build of the app and the Vite dev server started from this project.
$root = Split-Path -Parent $PSScriptRoot
$procs = Get-CimInstance Win32_Process | Where-Object {
  ($_.Name -eq "deckless.exe" -and $_.ExecutablePath -like "$root*") -or
  ($_.Name -eq "node.exe" -and $_.CommandLine -like "*$root*vite*")
}
foreach ($p in $procs) {
  Write-Output "stopping $($p.ProcessId) $($p.Name)"
  try { Stop-Process -Id $p.ProcessId -Force -ErrorAction Stop } catch {}
}
