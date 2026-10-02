# ============================================================================
# make-uiaccess.ps1 — сборка и установка UIAccess-версии keypress.
#
# Зачем: меню Пуск на Windows 11 (CoreWindow StartMenuExperienceHost) Windows
# держит в защищённой shell-полосе ВЫШЕ обычных topmost-окон, поэтому обычная
# сборка keypress под Пуском не видна, сколько ни переутверждай HWND_TOPMOST.
# Процесс с uiAccess="true" в манифесте получает собственную z-полосу UIAccess,
# расположенную выше полосы оболочки — так работает osk.exe (экранная
# клавиатура), который виден и при открытом Пуске.
#
# Что делает скрипт (запускать из папки проекта, PowerShell ОТ ИМЕНИ
# АДМИНИСТРАТОРА):
#   1. cargo build --release с KEYPRESS_UIACCESS=1
#      (build.rs вшивает манифест с uiAccess="true")
#   2. Создаёт самодписанный сертификат подписи кода и кладёт его в
#      доверенные корни + доверенных издателей локальной машины
#   3. Подписывает target\release\keypress.exe
#   4. Копирует exe в "$env:ProgramFiles\keypress" (uiAccess требует
#      защищённой папки — из Downloads/Desktop такой exe не запустится)
#
# Требования: Rust (rustup), PowerShell 5+.
# Конфиг: keypress.toml читается либо из папки exe (если доступна на запись),
# либо из %APPDATA%\keypress\keypress.toml — панель настроек работает
# без прав администратора.
# ============================================================================

$ErrorActionPreference = "Stop"

# --- Проверки окружения -----------------------------------------------------

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
           ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    throw "Запустите PowerShell от имени администратора (нужны запись в Program Files и хранилища сертификатов)."
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo не найден в PATH. Установите Rust: https://rustup.rs"
}

# --- 1/4: сборка ------------------------------------------------------------

Write-Host "=== 1/4: cargo build --release (KEYPRESS_UIACCESS=1) ===" -ForegroundColor Cyan
$env:KEYPRESS_UIACCESS = "1"
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "Сборка не удалась (см. вывод cargo выше)." }
Remove-Item Env:\KEYPRESS_UIACCESS

$exe = "target\release\keypress.exe"
if (-not (Test-Path $exe)) { throw "Не найден $exe" }

# --- 2/4: сертификат подписи ------------------------------------------------

$cert = Get-ChildItem Cert:\CurrentUser\My |
    Where-Object { $_.Subject -eq "CN=Keypress UIAccess" -and $_.HasPrivateKey } |
    Sort-Object NotAfter -Descending | Select-Object -First 1

if (-not $cert) {
    Write-Host "=== 2/4: создание сертификата подписи (CN=Keypress UIAccess) ===" -ForegroundColor Cyan
    $cert = New-SelfSignedCertificate -Type CodeSigningCert `
        -Subject "CN=Keypress UIAccess" `
        -CertStoreLocation "Cert:\CurrentUser\My" `
        -NotAfter (Get-Date).AddYears(10)

    # Без сертификата в доверенных издателях локальной машины Windows
    # откажется запускать UIAccess-exe (ошибка «плохая подпись» при старте).
    $pw  = ConvertTo-SecureString -String "keypress-local-export" -Force -AsPlainText
    $pfx = Join-Path $env:TEMP "keypress-sign.pfx"
    Export-PfxCertificate -Cert $cert -FilePath $pfx -Password $pw | Out-Null
    Import-PfxCertificate -FilePath $pfx -CertStoreLocation "Cert:\LocalMachine\Root" -Password $pw | Out-Null
    Import-PfxCertificate -FilePath $pfx -CertStoreLocation "Cert:\LocalMachine\TrustedPublisher" -Password $pw | Out-Null
    Remove-Item $pfx
} else {
    Write-Host "=== 2/4: используем существующий сертификат $($cert.Thumbprint) ===" -ForegroundColor Cyan
}

# --- 3/4: подпись ------------------------------------------------------------

Write-Host "=== 3/4: подпись keypress.exe ===" -ForegroundColor Cyan
$signed = Set-AuthenticodeSignature -FilePath $exe -Certificate $cert
if ($signed.Status -ne "Valid") { throw "Подпись не удалась: $($signed.StatusMessage)" }

# --- 4/4: установка -----------------------------------------------------------

$dest = Join-Path $env:ProgramFiles "keypress"
Write-Host "=== 4/4: установка в $dest ===" -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item $exe (Join-Path $dest "keypress.exe") -Force

Write-Host ""
Write-Host "Готово: $dest\keypress.exe" -ForegroundColor Green
Write-Host "Проверка подписи: Get-AuthenticodeSignature '$dest\keypress.exe' | Select Status"
Write-Host "Запускайте оверлей ярлыком оттуда. Ctrl+Alt+Q — выход, Ctrl+Alt+K — пауза."
Write-Host ""
Write-Host "Важно: UIAccess-версия НЕ сосуществует с обычной безболезненно —" 
Write-Host "остановите обычный оверлей перед запуском (или перезагрузитесь)."
