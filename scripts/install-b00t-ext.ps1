# install-b00t-ext.ps1 — Install b00t browser extension on Windows Chrome
# Run from PowerShell (Admin) or via: powershell.exe -File C:\b00t\install-b00t-ext.ps1

$extPath = "C:\b00t\browser-ext"
$manifest = Get-Content "$extPath\manifest.json" | ConvertFrom-Json

Write-Host "🥾 b00t Browser Extension v$($manifest.version)" -ForegroundColor Cyan
Write-Host ""

# ── Method 1: Registry force-install (requires extension ID + update URL) ──
# This works for enterprise deployments but needs a hosted .crx
# Uncomment and set your extension ID:
# $extId = "your-extension-id-here"
# $regPath = "HKLM:\Software\Google\Chrome\Extensions\$extId"
# New-Item -Path $regPath -Force | Out-Null
# New-ItemProperty -Path $regPath -Name "update_url" -Value "https://b00t.promptexecution.com/ext/update.xml" -Force

# ── Method 2: Chrome with --load-extension (development) ──
Write-Host "Launching Chrome with b00t extension loaded..." -ForegroundColor Green

$chromePaths = @(
    "C:\Program Files\Google\Chrome\Application\chrome.exe",
    "C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe"
)

$chrome = $null
foreach ($path in $chromePaths) {
    if (Test-Path $path) { $chrome = $path; break }
}

if (-not $chrome) {
    Write-Host "❌ Chrome not found. Install Chrome first." -ForegroundColor Red
    exit 1
}

# Kill existing Chrome (optional — comment out to keep sessions)
# Get-Process chrome -ErrorAction SilentlyContinue | Stop-Process -Force

Write-Host "  Chrome: $chrome"
Write-Host "  Extension: $extPath"
Write-Host ""

# Launch Chrome with extension and remote debugging
$args = @(
    "--load-extension=$extPath",
    "--remote-debugging-port=9222",
    "--remote-allow-origins=*",
    "--no-first-run",
    "http://localhost:31337/"
)

Write-Host "Starting Chrome..." -ForegroundColor Yellow
Start-Process -FilePath $chrome -ArgumentList $args

Write-Host "✅ Chrome launching with b00t extension" -ForegroundColor Green
Write-Host "   Dashboard: http://localhost:31337/"
Write-Host "   CDP: http://localhost:9222/"
Write-Host ""
Write-Host "💡 Tip: Pin the extension from chrome://extensions/ for persistent toolbar access"
