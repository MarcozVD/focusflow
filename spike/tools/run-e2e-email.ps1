# E2E del módulo de email: mocks IA + IMAP contra el binario release.
# Fases: 1) sync limpio -> sugerencias pending  2) trusted sender -> auto-aprobación
#        3) resync sin novedades -> checkpoint avanza (0 nuevos)  4) limpieza
$ErrorActionPreference = "Continue"
$spike = "C:\Users\mvale\focusflow\spike"
$exe = "$spike\src-tauri\target\release\focusflow-spike.exe"
$log = "$env:TEMP\focusflow-spike\spike.log"
$appData = "$env:APPDATA\com.focusflow.spike"

function Kill-App { Get-Process focusflow-spike -ErrorAction SilentlyContinue | Stop-Process -Force }
function Kill-Nodes { Get-CimInstance Win32_Process -Filter "Name='node.exe'" | Where-Object { $_.CommandLine -match "tools\\mock-" } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue } }
function Show-Log {
  Get-Content $log -ErrorAction SilentlyContinue | Select-String -Pattern "email_|sync|SYNC|suggestion|SUGGEST|EMAIL|auto_approved|TRUSTED|checkpoint|notification|new_sug" | ForEach-Object { $_.Line }
}

Write-Output "=== E2E EMAIL (mocks: IA 9410, IMAP 1143) ==="
Kill-Nodes
Kill-App
Start-Sleep 1
Remove-Item "$appData\focusflow.db", "$appData\focusflow.db-wal", "$appData\focusflow.db-shm" -Force -ErrorAction SilentlyContinue

# lanzar mocks persistentes (sin consola)
Start-Process -FilePath "node" -ArgumentList "tools/mock-ai.mjs" -WorkingDirectory $spike -WindowStyle Hidden
Start-Process -FilePath "node" -ArgumentList "tools/mock-imap.mjs" -WorkingDirectory $spike -WindowStyle Hidden
Start-Sleep 2
$ok1143 = (Test-NetConnection 127.0.0.1 -Port 1143 -WarningAction SilentlyContinue).TcpTestSucceeded
$ok9410 = (Test-NetConnection 127.0.0.1 -Port 9410 -WarningAction SilentlyContinue).TcpTestSucceeded
Write-Output "mocks: imap=$ok1143 ai=$ok9410"
if (-not $ok1143 -or -not $ok9410) { Write-Output "FALLO: mocks no escuchando"; exit 1 }

$emailCfg = '{"host":"127.0.0.1","port":1143,"ssl":false,"user":"usuario","mailboxes":["INBOX"],"senders":[],"domains":[],"keywords":[]}'
$env:AI_ENDPOINT = "http://127.0.0.1:9410"; $env:AI_MODEL = "mock-model"; $env:AI_API_KEY = "mock-key"
$env:FF_EMAIL_PASSWORD = "mock-pass"

# ---------- FASE 1: sync limpio -> 2 sugerencias pending ----------
Write-Output "`n--- FASE 1: primer sync (sin trusted) ---"
Remove-Item $log -Force -ErrorAction SilentlyContinue
$env:FF_EMAIL_CONFIG_JSON = $emailCfg
$env:FF_SYNC_NOW = "1"
$env:FF_WIDGET = "0"; $env:FF_NOTIFY = "0"
Start-Process -FilePath $exe
Start-Sleep 14
Remove-Item Env:FF_EMAIL_CONFIG_JSON, Env:FF_SYNC_NOW, Env:FF_WIDGET, Env:FF_NOTIFY
Show-Log

# ---------- FASE 2: trusted sender -> auto-aprobación ----------
Write-Output "`n--- FASE 2: resync con jefa@empresa.test de confianza ---"
Kill-App
Start-Sleep 2
Remove-Item $log -Force -ErrorAction SilentlyContinue
$env:FF_TRUSTED_ADD = "jefa@empresa.test"
$env:FF_SYNC_NOW = "1"
$env:FF_WIDGET = "0"; $env:FF_NOTIFY = "0"
Start-Process -FilePath $exe
Start-Sleep 14
Remove-Item Env:FF_TRUSTED_ADD, Env:FF_SYNC_NOW, Env:FF_WIDGET, Env:FF_NOTIFY
Show-Log

# ---------- FASE 3: resync sin novedades -> checkpoint ----------
Write-Output "`n--- FASE 3: resync sin novedades ---"
Kill-App
Start-Sleep 2
Remove-Item $log -Force -ErrorAction SilentlyContinue
$env:FF_SYNC_NOW = "1"
$env:FF_WIDGET = "0"; $env:FF_NOTIFY = "0"
Start-Process -FilePath $exe
Start-Sleep 14
Remove-Item Env:FF_SYNC_NOW, Env:FF_WIDGET, Env:FF_NOTIFY
Show-Log

# ---------- limpieza ----------
Kill-App
Kill-Nodes
Remove-Item Env:AI_ENDPOINT, Env:AI_MODEL, Env:AI_API_KEY, Env:FF_EMAIL_PASSWORD -ErrorAction SilentlyContinue
Write-Output "`n=== E2E EMAIL FIN ==="