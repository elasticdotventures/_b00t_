#!/usr/bin/env pwsh
# 🥾 b00t Windows Tray Diagnostic
# =================================
# Tests: binary existence, Start Menu, registry, process, tray icon API
# Usage: powershell -File scripts/detect-windows-tray.ps1
#        powershell -File scripts/detect-windows-tray.ps1 -Repair

param(
    [switch]$Repair,
    [switch]$Verbose
)

$PASS = "✅"
$FAIL = "❌"
$WARN = "⚠️"
$PASS_COUNT = 0
$FAIL_COUNT = 0
$WARN_COUNT = 0

function Test-Step {
    param($Name, $Condition, $FixHint)
    if (& $Condition) {
        Write-Host "  $PASS $Name"
        $script:PASS_COUNT++
    } else {
        Write-Host "  $FAIL $Name"
        if ($FixHint) { Write-Host "       $FixHint" }
        $script:FAIL_COUNT++
    }
}

function Warn-Step {
    param($Name, $Message)
    Write-Host "  $WARN $Name"
    Write-Host "       $Message"
    $script:WARN_COUNT++
}

Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════╗"
Write-Host "║  🥾 b00t Windows Tray Diagnostic                  ║"
Write-Host "╚═══════════════════════════════════════════════════╝"
Write-Host ""

# ─── 1. Binary Check ──────────────────────────────────────────────────────

Write-Host "📍 Binary Installation"
Write-Host "   ─────────────────────"

Test-Step -Name "ledgerr-tauri.exe exists" -Condition {
    Test-Path "C:\b00t\ledgerr-tauri.exe"
} -FixHint "Run: just install-tauri (from WSL)"

Test-Step -Name "Binary is runnable" -Condition {
    try {
        $ver = & "C:\b00t\ledgerr-tauri.exe" --version 2>&1
        $LASTEXITCODE -eq 0
    } catch { $false }
} -FixHint "Binary may be corrupted. Reinstall: just install-tauri"

# ─── 2. Start Menu ────────────────────────────────────────────────────────

Write-Host ""
Write-Host "📍 Start Menu Registration"
Write-Host "   ─────────────────────────"

$startMenuPath = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\b00t.lnk"
Test-Step -Name "Start Menu shortcut exists" -Condition {
    Test-Path $startMenuPath
} -FixHint "Run: just install-tauri (from WSL) to create shortcut"

if (Test-Path $startMenuPath) {
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($startMenuPath)
    Test-Step -Name "Shortcut targets correct binary" -Condition {
        $shortcut.TargetPath -eq "C:\b00t\ledgerr-tauri.exe"
    } -FixHint "Shortcut points to: $($shortcut.TargetPath). Reinstall."
}

# ─── 3. Process Check ─────────────────────────────────────────────────────

Write-Host ""
Write-Host "📍 Process Status"
Write-Host "   ───────────────"

$proc = Get-Process -Name "ledgerr-tauri" -ErrorAction SilentlyContinue
Test-Step -Name "ledgerr-tauri process is running" -Condition { $null -ne $proc } -FixHint "Run: C:\b00t\ledgerr-tauri.exe (double-click)"

if ($null -ne $proc) {
    Warn-Step -Name "Process details" -Message "PID: $($proc.Id) | Start: $($proc.StartTime) | CPU: $($proc.CPU)s | Mem: $([math]::Round($proc.WorkingSet64 / 1MB, 1))MB"
}

# ─── 4. Tray Icon via Win32 API ───────────────────────────────────────────

Write-Host ""
Write-Host "📍 Tray Icon Verification"
Write-Host "   ────────────────────────"

# Check if the process has a hidden window (Shell_NotifyIconW requires one)
if ($null -ne $proc) {
    $hWnd = $proc.MainWindowHandle
    if ($hWnd -eq 0) {
        Warn-Step -Name "Main window handle" -Message "MainWindowHandle is 0 (expected for tray-only apps)"
    }

    # Check for NOTIFYICONDATA via Win32 API (P/Invoke)
    Add-Type @"
        using System;
        using System.Runtime.InteropServices;
        public class TrayCheck {
            [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
            public static extern bool Shell_NotifyIconW(int dwMessage, ref NOTIFYICONDATA lpData);

            public struct NOTIFYICONDATA {
                public int cbSize;
                public IntPtr hWnd;
                public int uID;
                public int uFlags;
                public int uCallbackMessage;
                public IntPtr hIcon;
                [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
                public string szTip;
                public int dwState;
                public int dwStateMask;
                [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)]
                public string szInfo;
                public int uTimeout;
                public int uVersion;
            }

            public static uint NIM_QUERY = 5;
        }
"@

    # We can't directly query another process's tray icons without injecting,
    # but we can check if our binary has the expected tray-related imports
    $binPath = "C:\b00t\ledgerr-tauri.exe"
    if (Test-Path $binPath) {
        $bytes = [System.IO.File]::ReadAllBytes($binPath)
        $text = [System.Text.Encoding]::UTF8.GetString($bytes)
        
        Test-Step -Name "Binary contains Shell_NotifyIconW" -Condition {
            $text -match "Shell_NotifyIcon"
        } -FixHint "Binary was built without Windows tray support. Rebuild with: cargo build --release -p ledgerr-tauri"
        
        Test-Step -Name "Binary has tray menu items" -Condition {
            ($text -match "Show Window") -and ($text -match "Exit")
        } -FixHint "Tray menu not configured in the build"
    }
} else {
    # Binary check without process running
    $binPath = "C:\b00t\ledgerr-tauri.exe"
    if (Test-Path $binPath) {
        $bytes = [System.IO.File]::ReadAllBytes($binPath)
        $text = [System.Text.Encoding]::UTF8.GetString($bytes)
        
        Test-Step -Name "Binary contains Shell_NotifyIconW" -Condition {
            $text -match "Shell_NotifyIcon"
        } -FixHint "Binary was built without Windows tray support."
        
        Test-Step -Name "Binary has tray menu items" -Condition {
            ($text -match "Show Window") -and ($text -match "Exit")
        } -FixHint "Tray menu not configured."
    }
}

# ─── 5. Registry Check ────────────────────────────────────────────────────

Write-Host ""
Write-Host "📍 Registry Settings"
Write-Host "   ───────────────────"

$regPath = "HKCU:\Software\b00t"
Test-Step -Name "b00t registry key exists" -Condition {
    Test-Path $regPath
} -FixHint "Registry key will be created when ledgerr-tauri runs and saves settings"

# ─── 6. Summary ───────────────────────────────────────────────────────────

Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════╗"
Write-Host "║  Summary                                          ║"
Write-Host "╠═══════════════════════════════════════════════════╣"
Write-Host "║  $PASS  Passed:  $PASS_COUNT                                      ║"
Write-Host "║  $FAIL  Failed:  $FAIL_COUNT                                      ║"
Write-Host "║  $WARN  Warnings: $WARN_COUNT                                      ║"
Write-Host "╚═══════════════════════════════════════════════════╝"
Write-Host ""

if ($FAIL_COUNT -gt 0) {
    Write-Host "🔧 Fix:"
    if (-not (Test-Path "C:\b00t\ledgerr-tauri.exe")) {
        Write-Host "   1. From WSL: just install-tauri"
        Write-Host "   2. From Windows: cd vendor\ledgrrr && cargo build --release -p ledgerr-tauri"
        Write-Host "      then copy target\release\ledgerr-tauri.exe to C:\b00t\"
    }
    if (-not (Get-Process "ledgerr-tauri" -ErrorAction SilentlyContinue)) {
        Write-Host "   3. Run: C:\b00t\ledgerr-tauri.exe"
    }
    Write-Host ""
    Write-Host "   Then re-run this diagnostic to verify."
} elseif ($PASS_COUNT -gt 0) {
    Write-Host "🎉 All checks passed. b00t should be in your system tray (🥾)."
    Write-Host "   If you don't see it, try:"
    Write-Host "   - Click the arrow (^) in the taskbar to expand hidden icons"
    Write-Host "   - Drag the 🥾 icon to the taskbar to pin it permanently"
}

# Repair mode
if ($Repair) {
    Write-Host ""
    Write-Host "🔧 Repair mode..."
    if (-not (Test-Path "C:\b00t\ledgerr-tauri.exe")) {
        Write-Host "  $FAIL Cannot repair — binary missing. Run 'just install-tauri' from WSL."
    } else {
        # Kill any existing process
        Get-Process "ledgerr-tauri" -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep 1
        # Restart
        Start-Process "C:\b00t\ledgerr-tauri.exe" -WindowStyle Hidden
        Write-Host "  $PASS Restarted ledgerr-tauri.exe"
    }
}

Write-Host ""
Write-Host "Done."
