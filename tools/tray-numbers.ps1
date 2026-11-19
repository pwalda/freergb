# SPDX-License-Identifier: GPL-2.0-or-later
# Tray numbers of the real app on Windows: memory and CPU of freergb.exe, plus any WebView2
# processes it started (there should be none once the window is closed), sampled every few
# seconds. Close the window first to measure tray mode; run with and without an effect.
#
#   powershell -ExecutionPolicy Bypass -File tools\tray-numbers.ps1 [-Minutes 5] [-Every 5]
param([double]$Minutes = 5, [int]$Every = 5)

# The installed exe is freeRGB.exe; a dev build is freergb-app.exe. OpenRGB and the sensor
# service also match *rgb*, so they are left out by name.
$app = Get-Process | Where-Object { $_.Name -in 'freeRGB', 'freergb', 'freergb-app' } | Select-Object -First 1
if (-not $app) {
    $seen = (Get-Process | Where-Object { $_.Name -like '*rgb*' } | ForEach-Object { $_.Name }) -join ', '
    Write-Error "freeRGB is not running (processes with 'rgb' in the name: $(if ($seen) { $seen } else { 'none' }))"
    exit 1
}

function Children($parentId) {
    Get-CimInstance Win32_Process -Filter "ParentProcessId=$parentId" | ForEach-Object {
        $_.ProcessId
        Children $_.ProcessId
    }
}

$cores = [Environment]::ProcessorCount
$samples = @()
$end = (Get-Date).AddMinutes($Minutes)
$prevCpu = $app.TotalProcessorTime.TotalSeconds
$prevAt = Get-Date
while ((Get-Date) -lt $end) {
    Start-Sleep -Seconds $Every
    $app.Refresh()
    if ($app.HasExited) { Write-Error 'freergb exited'; exit 1 }
    $now = Get-Date
    $cpu = $app.TotalProcessorTime.TotalSeconds
    # Percent of one core, like the soak harness; Task Manager divides by all cores.
    $pct = 100 * ($cpu - $prevCpu) / ($now - $prevAt).TotalSeconds
    $prevCpu, $prevAt = $cpu, $now
    $kids = @(Children $app.Id | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
    $kidMb = ($kids | Measure-Object WorkingSet64 -Sum).Sum / 1MB
    $s = [pscustomobject]@{
        Time        = $now.ToString('HH:mm:ss')
        'App MB'    = [math]::Round($app.WorkingSet64 / 1MB, 1)
        'Private MB'= [math]::Round($app.PrivateMemorySize64 / 1MB, 1)
        'CPU %core' = [math]::Round($pct, 2)
        Children    = $kids.Count
        'Child MB'  = [math]::Round($kidMb, 1)
    }
    $samples += $s
    $s | Format-Table -HideTableHeaders:($samples.Count -gt 1) | Out-String -Stream | Where-Object { $_ }
}

$mb = $samples | Measure-Object 'App MB' -Average -Maximum
$cpu = $samples | Measure-Object 'CPU %core' -Average
"`nfreergb.exe: working set avg {0:N1} MB, max {1:N1} MB; CPU avg {2:N2}% of one core ({3:N2}% of all {4} cores)" -f $mb.Average, $mb.Maximum, $cpu.Average, ($cpu.Average / $cores), $cores
"Child processes at the end: {0} (OpenRGB, if freergb started it, is one; WebView2 should be gone in tray mode)" -f $samples[-1].Children
