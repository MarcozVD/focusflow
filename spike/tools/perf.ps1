# Perf de app (fase 13): tiempo de arranque + memoria.
# Uso: pwsh tools/perf.ps1
# Mide: ms hasta la primera línea de log (proceso listo) y RSS/private tras 10 s.

$exe = Join-Path $PSScriptRoot "..\src-tauri\target\debug\focusflow-spike.exe"
$log = Join-Path $env:TEMP "focusflow-spike\spike.log"
$old = if (Test-Path $log) { (Get-Item $log).Length } else { 0 }

Get-Process focusflow-spike -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

$t0 = Get-Date
$p = Start-Process -FilePath $exe -PassThru

# esperar a que el proceso escriba en el log (setup + scheduler arrancado)
$ready = $false
for ($i = 0; $i -lt 120; $i++) {
    Start-Sleep -Milliseconds 250
    if (Test-Path $log) {
        if ((Get-Item $log).Length -gt $old) { $ready = $true; break }
    }
    if ($p.HasExited) { Write-Host "FATAL: proceso salió con código $($p.ExitCode)"; exit 1 }
}
if (-not $ready) { Write-Host "FATAL: sin log tras 30 s"; exit 1 }
$t1 = Get-Date
$startup_ms = [math]::Round(($t1 - $t0).TotalMilliseconds)

# dejar estabilizar el motor de sync + webviews
Start-Sleep -Seconds 10
$p.Refresh()
$ws = [math]::Round($p.WorkingSet64 / 1MB, 1)
$pm = $p.PrivateMemorySize64
$priv = [math]::Round($pm / 1MB, 1)

Write-Host ""
Write-Host "===== FocusFlow app perf (debug build) ====="
Write-Host ("startup (exe -> primera línea de log): {0} ms" -f $startup_ms)
Write-Host ("working set (10 s tras arranque):       {0} MB" -f $ws)
Write-Host ("private bytes (10 s tras arranque):     {0} MB" -f $priv)
Write-Host "============================================"
Write-Host ("últimas líneas del log:")
Get-Content $log -Tail 3 | ForEach-Object { "  $_" }
