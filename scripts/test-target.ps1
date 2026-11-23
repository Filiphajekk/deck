# Dev helper: a plain window with a multi-line Win32 edit control used as a target for
# acceptance tests (select all, typed text). It mirrors its text and selection into -Out.
param([string]$Out = "target.txt", [string]$Text = "hello world", [int]$Seconds = 60)
Add-Type -AssemblyName System.Windows.Forms
$form = New-Object System.Windows.Forms.Form
$form.Text = "Deckless test target"
$form.Width = 700; $form.Height = 300; $form.StartPosition = "CenterScreen"; $form.TopMost = $true
$box = New-Object System.Windows.Forms.RichTextBox
$box.Dock = "Fill"; $box.Font = New-Object System.Drawing.Font("Segoe UI", 14)
$box.Text = $Text; $box.SelectionStart = $box.Text.Length; $box.SelectionLength = 0
$form.Controls.Add($box)
$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 100
$start = Get-Date
$timer.Add_Tick({
  $state = "sel=$($box.SelectionStart),$($box.SelectionLength)`n$($box.Text)"
  [System.IO.File]::WriteAllText($Out, $state, [System.Text.Encoding]::UTF8)
  if (((Get-Date) - $start).TotalSeconds -gt $Seconds) { $form.Close() }
})
$timer.Start()
$form.Add_Shown({ $form.Activate(); $box.Focus() })
[System.Windows.Forms.Application]::Run($form)
