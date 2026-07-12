#!/usr/bin/env pwsh
# 🥾 List all system tray items — find out what's in the notification area

Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════╗"
Write-Host "║  Windows System Tray — Current Items              ║"
Write-Host "╚═══════════════════════════════════════════════════╝"
Write-Host ""

# Method 1: All background processes with no visible window (potential tray icons)
Write-Host "📍 Background processes (potential tray icons):"
Write-Host "   (sorted by memory usage, top 30)"
$bg = Get-Process | Where-Object { $_.MainWindowHandle -eq 0 } | Sort-Object WorkingSet64 -Descending | Select-Object -First 30
$bg | ForEach-Object {
    $mem = [math]::Round($_.WorkingSet64 / 1MB, 0)
    $name = $_.ProcessName
    $pid = $_.Id
    if ($name -eq 'ledgerr-tauri') {
        Write-Host "   ✅ b00t (ledgerr-tauri) PID:$pid $mem`MB 🥾" -ForegroundColor Green
    } else {
        Write-Host "      $name PID:$pid $mem`MB"
    }
}

# Method 2: Check Explorer notification area
Write-Host ""
Write-Host "📍 Explorer notification area (pinned icons):"
$explorerPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Taskband"
if (Test-Path $explorerPath) {
    $items = Get-ItemProperty $explorerPath
    $items.PSObject.Properties | Where-Object { $_.Name -like "*Favorites*" -or $_.Name -like "*Notification*" } | ForEach-Object {
        Write-Host "   $($_.Name) = $($_.Value)"
    }
} else {
    Write-Host "   (no taskband key)"
}

# Method 3: Check for b00t specifically
Write-Host ""
$b00t = Get-Process -Name "ledgerr-tauri" -ErrorAction SilentlyContinue
if ($b00t) {
    $mem = [math]::Round($b00t.WorkingSet64 / 1MB, 1)
    Write-Host "🥾 b00t IS RUNNING — PID:$($b00t.Id) $mem`MB"
    Write-Host "   Check your taskbar notification area (bottom-right)"
    Write-Host "   Click the ^ arrow to expand hidden icons"
    Write-Host "   Look for the 🥾 boot icon"
    Write-Host "   If hidden, drag it to the taskbar to pin"
} else {
    Write-Host "❌ b00t (ledgerr-tauri) is NOT running"
    Write-Host ""
    Write-Host "Possible reasons:"
    if (-not (Test-Path "C:\b00t\ledgerr-tauri.exe")) {
        Write-Host "  1. Binary not installed at C:\b00t\ledgerr-tauri.exe"
        Write-Host "     → Build: cd vendor\ledgrrr && cargo build --release -p ledgerr-tauri"
        Write-Host "     → Copy: copy target\release\ledgerr-tauri.exe C:\b00t\"
    }
    Write-Host "  2. Not started → run: C:\b00t\ledgerr-tauri.exe"
    Write-Host "  3. Crashed → check Event Viewer > Windows Logs > Application"
    Write-Host ""
    Write-Host "To install + start from scratch:"
    Write-Host "  cd vendor\ledgrrr"
    Write-Host "  cargo build --release -p ledgerr-tauri"
    Write-Host "  mkdir C:\b00t 2>nul"
    Write-Host "  copy target\release\ledgerr-tauri.exe C:\b00t\"
    Write-Host "  C:\b00t\ledgerr-tauri.exe"
    Write-Host ""
    Write-Host "Then re-run this script to verify:"
    Write-Host "  powershell -File scripts\list-tray-items.ps1"
}

Write-Host ""
Write-Host "Done."
