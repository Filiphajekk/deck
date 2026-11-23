# Downloads and starts the Deckless installer.
#
#   irm https://raw.githubusercontent.com/Filiphajekk/deck/main/install.ps1 | iex
#
# Silent install (no installer window):
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/Filiphajekk/deck/main/install.ps1))) -Silent
param([switch]$Silent)

$ErrorActionPreference = "Stop"
$url = "https://github.com/Filiphajekk/deck/raw/main/Deckless-Setup.exe"
$out = Join-Path $env:TEMP "Deckless-Setup.exe"

Write-Host "Downloading Deckless..."
$ProgressPreference = "SilentlyContinue"   # much faster downloads in Windows PowerShell
Invoke-WebRequest -Uri $url -OutFile $out -UseBasicParsing

Write-Host "Installing..."
if ($Silent) {
  Start-Process -FilePath $out -ArgumentList "/S" -Wait
  $exe = Join-Path $env:LOCALAPPDATA "Deckless\deckless.exe"
  if (Test-Path $exe) { Start-Process $exe }
} else {
  Start-Process -FilePath $out -Wait
}
Remove-Item $out -ErrorAction SilentlyContinue
Write-Host "Done. Press Ctrl+Shift+Space to open the panel."
