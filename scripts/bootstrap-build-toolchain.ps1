#!/usr/bin/env pwsh
# 🥾 b00t Build Toolchain Bootstrap
# ===================================
# Detects and resolves missing native build dependencies for this
# workspace's Rust crates: a C linker ('cc' / 'link.exe'), libclang
# (bindgen — e.g. oxrocksdb-sys), and the other *-sys build inputs
# (cmake, nasm, perl, pkg-config) that show up across this workspace's
# Cargo.lock (aws-lc-sys, zstd-sys, ring, oxrocksdb-sys, ...).
#
# Cross-platform (Windows / Linux incl. WSL / macOS) — run it on whichever
# machine is missing a build. Requires PowerShell 7+ (`pwsh`); on Linux,
# install it first via your package manager if it isn't already present.
#
# Born from two real gaps hit in the same session, on two different
# machines, both root-caused to "this dependency was never provisioned":
#   - WSL dev machine: `cargo check` failed with `linker \`cc\` not found`
#     — no build-essential installed (l3dg3rr#212 / ledgrrr#235 review).
#   - Windows dev machine: `store-oxigraph`'s oxrocksdb-sys failed its
#     bindgen step with "Unable to find libclang" (_b00t_#1177 P4 scoping
#     report) — LLVM/libclang never installed there.
#
# Usage:
#   pwsh scripts/bootstrap-build-toolchain.ps1                 # detect + fix, host target
#   pwsh scripts/bootstrap-build-toolchain.ps1 -DryRun          # detect + print plan only
#   pwsh scripts/bootstrap-build-toolchain.ps1 -Yes             # no prompts (CI / unattended)
#   pwsh scripts/bootstrap-build-toolchain.ps1 -Target x86_64-pc-windows-gnu
#
# Exit code is non-zero if any REQUIRED dependency is still missing when
# the script finishes (after fixes, or always under -DryRun).

param(
    # Rust target triple to provision for. Empty = auto-detect from the
    # host (via `rustc -vV` if rustc is already installed, else a
    # per-OS default). Configurable because the *C toolchain* choice is
    # target-specific on Windows: `-pc-windows-msvc` needs Visual Studio
    # Build Tools, `-pc-windows-gnu` needs a mingw-w64 gcc instead — same
    # OS, different fix. Other requirements below (libclang, cmake, ...)
    # aren't target-specific, so -Target only changes the C-toolchain step.
    [string]$Target = "",

    # Detect and print the plan; install nothing.
    [switch]$DryRun,

    # Skip interactive confirmation prompts (installs proceed automatically).
    [switch]$Yes,

    [switch]$Verbose
)

$ErrorActionPreference = "Stop"

$PASS = "✅"; $FAIL = "❌"; $WARN = "⚠️"; $STEP = "🔧"
$script:FixesApplied = 0
$script:StillMissing = @()

function Write-Section($title) {
    Write-Host ""
    Write-Host "── $title ──" -ForegroundColor Cyan
}

function Test-CommandExists([string]$name) {
    return [bool](Get-Command $name -ErrorAction SilentlyContinue)
}

function Update-SessionPath {
    # Winget-installed tools land on the Machine/User PATH but this
    # process's $env:PATH was captured at launch — re-read both so a
    # freshly installed tool is visible to the *same run's* later checks
    # without requiring the user to open a new shell.
    if (-not $script:OnWindows) { return }
    $machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $user = [Environment]::GetEnvironmentVariable("Path", "User")
    $env:Path = "$machine;$user"
}

function Confirm-Action([string]$message) {
    if ($Yes -or $DryRun) { return $true }
    $resp = Read-Host "$message [Y/n]"
    return ($resp -eq "" -or $resp -match "^[Yy]")
}

# Runs one requirement: $TestScript returns $true if already satisfied;
# $FixScript (if given) performs the fix. Tallies pass/fixed/still-missing
# and never throws — a failed fix is reported, not fatal to the rest of
# the run, so one blocked step doesn't hide the status of everything else.
function Test-Requirement {
    param(
        [Parameter(Mandatory)] [string]$Name,
        [Parameter(Mandatory)] [scriptblock]$TestScript,
        [scriptblock]$FixScript = $null,
        [string]$FixDescription = "",
        [string]$ManualHint = "",
        [bool]$Required = $true
    )

    if (& $TestScript) {
        Write-Host "  $PASS $Name"
        return
    }

    if (-not $FixScript) {
        $marker = if ($Required) { $FAIL } else { $WARN }
        Write-Host "  $marker $Name — missing"
        if ($ManualHint) { Write-Host "     $ManualHint" -ForegroundColor DarkGray }
        if ($Required) { $script:StillMissing += $Name }
        return
    }

    Write-Host "  $WARN $Name — missing"
    if ($FixDescription) { Write-Host "     $STEP $FixDescription" }

    if ($DryRun) {
        Write-Host "     (dry-run: would install now)" -ForegroundColor DarkGray
        if ($Required) { $script:StillMissing += $Name }
        return
    }

    if (-not (Confirm-Action "     Install now?")) {
        Write-Host "     ⏭  skipped"
        if ($Required) { $script:StillMissing += $Name }
        return
    }

    try {
        & $FixScript
        Update-SessionPath
        if (& $TestScript) {
            Write-Host "  $PASS $Name — installed"
            $script:FixesApplied++
        } else {
            Write-Host "  $FAIL $Name — install ran but requirement still not detected"
            Write-Host "     (may need a new shell for PATH changes to take effect)" -ForegroundColor DarkGray
            if ($Required) { $script:StillMissing += $Name }
        }
    } catch {
        Write-Host "  $FAIL $Name — install failed: $_" -ForegroundColor Red
        if ($ManualHint) { Write-Host "     $ManualHint" -ForegroundColor DarkGray }
        if ($Required) { $script:StillMissing += $Name }
    }
}

# ── Platform detection ──────────────────────────────────────────────────
# $IsWindows/$IsLinux/$IsMacOS only exist on pwsh 6+; Windows PowerShell
# 5.1 has no such variables and only ever runs on Windows.
$script:OnWindows = if (Get-Variable IsWindows -ErrorAction SilentlyContinue) { $IsWindows } else { $true }
$script:OnMacOS = [bool](Get-Variable IsMacOS -ErrorAction SilentlyContinue) -and $IsMacOS
$script:OnLinux = [bool](Get-Variable IsLinux -ErrorAction SilentlyContinue) -and $IsLinux

$osName = if ($OnWindows) { "Windows" } elseif ($OnMacOS) { "macOS" } elseif ($OnLinux) { "Linux" } else { "unknown" }

if (-not $Target) {
    if (Test-CommandExists rustc) {
        $hostLine = (& rustc -vV) | Select-String '^host:\s*(.+)$'
        if ($hostLine) { $Target = $hostLine.Matches[0].Groups[1].Value.Trim() }
    }
    if (-not $Target) {
        if ($OnWindows) {
            $Target = "x86_64-pc-windows-msvc"
        } elseif ($OnMacOS) {
            $arch = (& uname -m 2>$null)
            $Target = if ($arch -eq "arm64") { "aarch64-apple-darwin" } else { "x86_64-apple-darwin" }
        } else {
            $arch = (& uname -m 2>$null)
            $Target = if ($arch -eq "aarch64") { "aarch64-unknown-linux-gnu" } else { "x86_64-unknown-linux-gnu" }
        }
    }
}
$isGnuTarget = $Target -match "-gnu$"

Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════╗"
Write-Host "║  🥾 b00t Build Toolchain Bootstrap                ║"
Write-Host "╚═══════════════════════════════════════════════════╝"
Write-Host "  OS: $osName   Target: $Target" -ForegroundColor DarkGray
if ($DryRun) { Write-Host "  (dry-run — detect only, nothing will be installed)" -ForegroundColor Yellow }

# ── winget availability (Windows install path) ──────────────────────────
$wingetOk = -not $OnWindows -or (Test-CommandExists winget)
if ($OnWindows -and -not $wingetOk) {
    Write-Section "Package manager"
    Write-Host "  $FAIL winget not found"
    Write-Host "     Install 'App Installer' from the Microsoft Store, then re-run this script." -ForegroundColor DarkGray
    Write-Host "     (winget can't reliably bootstrap itself — this one's manual.)" -ForegroundColor DarkGray
}

# ── Rust toolchain (rustup + cargo + target) ─────────────────────────────
Write-Section "Rust toolchain"

Test-Requirement -Name "rustup" -TestScript { Test-CommandExists rustup } `
    -FixDescription "install via $(if ($OnWindows) {'winget (Rustlang.Rustup)'} else {'rustup.rs'})" `
    -ManualHint "https://rustup.rs" `
    -FixScript {
        if ($OnWindows) {
            if (-not $wingetOk) { throw "winget unavailable — see above" }
            winget install --id Rustlang.Rustup -e --accept-package-agreements --accept-source-agreements
        } else {
            & curl.exe --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs 2>$null | & sh -s -- -y
            if ($LASTEXITCODE -ne 0) {
                curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            }
            $env:PATH = "$HOME/.cargo/bin:$env:PATH"
        }
    }

Test-Requirement -Name "cargo" -TestScript { Test-CommandExists cargo } `
    -ManualHint "should come with rustup above — re-run this script after it installs"

if (Test-CommandExists rustup) {
    Test-Requirement -Name "rust target '$Target'" -TestScript {
        (& rustup target list --installed) -contains $Target
    } -FixDescription "rustup target add $Target" -FixScript {
        rustup target add $Target
    }
}

# ── C linker / compiler ('cc') ────────────────────────────────────────────
Write-Section "C linker/compiler ('cc')"

if ($OnWindows -and -not $isGnuTarget) {
    # MSVC targets link via link.exe from Visual Studio Build Tools, not a
    # literal 'cc' binary — vswhere is the standard way to check whether
    # the VC.Tools workload (which provides it) is present.
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    Test-Requirement -Name "MSVC Build Tools (link.exe / cl.exe)" -TestScript {
        if (Test-Path $vswhere) {
            $found = & $vswhere -latest -products * `
                -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
                -property installationPath
            return [bool]$found
        }
        return (Test-CommandExists cl) -or (Test-CommandExists link)
    } -FixDescription "winget install Microsoft.VisualStudio.2022.BuildTools (C++ workload)" `
      -ManualHint "https://visualstudio.microsoft.com/visual-cpp-build-tools/ — select 'Desktop development with C++'" `
      -FixScript {
        if (-not $wingetOk) { throw "winget unavailable — see above" }
        winget install --id Microsoft.VisualStudio.2022.BuildTools -e `
            --accept-package-agreements --accept-source-agreements `
            --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    }
} elseif ($OnWindows -and $isGnuTarget) {
    # windows-gnu target: rustc invokes gcc.exe directly, mingw-w64 style.
    Test-Requirement -Name "mingw-w64 gcc (windows-gnu target)" -TestScript {
        Test-CommandExists gcc
    } -FixDescription "winget install BrechtSanders.WinLibs.POSIX.UCRT (mingw-w64 gcc)" `
      -ManualHint "https://winlibs.com/ — add its bin/ to PATH manually if winget install doesn't" `
      -FixScript {
        if (-not $wingetOk) { throw "winget unavailable — see above" }
        winget install --id BrechtSanders.WinLibs.POSIX.UCRT -e `
            --accept-package-agreements --accept-source-agreements
    }
} elseif ($OnMacOS) {
    Test-Requirement -Name "Xcode Command Line Tools (cc)" -TestScript {
        (& xcode-select -p 2>$null); $LASTEXITCODE -eq 0
    } -FixDescription "xcode-select --install (opens a GUI installer — finish it, then re-run)" `
      -FixScript {
        xcode-select --install
        throw "Xcode CLT install was launched in a GUI dialog — finish it, then re-run this script."
    }
} else {
    # Linux (incl. WSL). This is the exact gap this script exists for:
    # `linker \`cc\` not found` when build-essential was never installed.
    $pkgManager = if (Test-CommandExists apt-get) { "apt" }
        elseif (Test-CommandExists dnf) { "dnf" }
        elseif (Test-CommandExists pacman) { "pacman" }
        else { $null }

    Test-Requirement -Name "cc (build-essential / gcc)" -TestScript {
        Test-CommandExists cc
    } -FixDescription "sudo $(
        switch ($pkgManager) {
            "apt" { "apt-get install -y build-essential" }
            "dnf" { "dnf group install -y 'Development Tools'" }
            "pacman" { "pacman -S --noconfirm base-devel" }
            default { "<no supported package manager found>" }
        }
    )" -ManualHint "install your distro's C toolchain package (build-essential / Development Tools / base-devel)" `
      -FixScript {
        switch ($pkgManager) {
            "apt" { sudo apt-get update; sudo apt-get install -y build-essential }
            "dnf" { sudo dnf group install -y "Development Tools" }
            "pacman" { sudo pacman -S --noconfirm base-devel }
            default { throw "no supported package manager (apt-get/dnf/pacman) found on PATH" }
        }
    }
}

# ── libclang (bindgen — oxrocksdb-sys and friends) ────────────────────────
Write-Section "libclang (bindgen)"

if ($OnWindows) {
    Test-Requirement -Name "LLVM / libclang" -TestScript {
        if ($env:LIBCLANG_PATH -and (Test-Path "$($env:LIBCLANG_PATH)\libclang.dll")) { return $true }
        return Test-Path "${env:ProgramFiles}\LLVM\bin\libclang.dll"
    } -FixDescription "winget install LLVM.LLVM, then set LIBCLANG_PATH" `
      -ManualHint "https://github.com/llvm/llvm-project/releases" `
      -FixScript {
        if (-not $wingetOk) { throw "winget unavailable — see above" }
        winget install --id LLVM.LLVM -e --accept-package-agreements --accept-source-agreements
        $llvmBin = "${env:ProgramFiles}\LLVM\bin"
        [Environment]::SetEnvironmentVariable("LIBCLANG_PATH", $llvmBin, "User")
        $env:LIBCLANG_PATH = $llvmBin
    }
} elseif ($OnMacOS) {
    Test-Requirement -Name "libclang (Xcode CLT / Homebrew LLVM)" -TestScript {
        (Test-CommandExists clang) -or (Test-Path "/Library/Developer/CommandLineTools/usr/lib/libclang.dylib")
    } -Required $false `
      -ManualHint "usually ships with Xcode Command Line Tools above; if bindgen still can't find it: brew install llvm"
} else {
    $pkgManager = if (Test-CommandExists apt-get) { "apt" }
        elseif (Test-CommandExists dnf) { "dnf" }
        elseif (Test-CommandExists pacman) { "pacman" }
        else { $null }

    Test-Requirement -Name "libclang-dev" -TestScript {
        (Test-CommandExists clang) -or (Test-Path "/usr/lib/llvm-*/lib/libclang.so*") -or
        ((& ldconfig -p 2>$null) -match "libclang\.so")
    } -FixDescription "sudo $(
        switch ($pkgManager) {
            "apt" { "apt-get install -y clang libclang-dev" }
            "dnf" { "dnf install -y clang clang-devel" }
            "pacman" { "pacman -S --noconfirm clang" }
            default { "<no supported package manager found>" }
        }
    )" -ManualHint "needed by bindgen-based *-sys crates, e.g. oxrocksdb-sys (store-oxigraph feature)" `
      -FixScript {
        switch ($pkgManager) {
            "apt" { sudo apt-get update; sudo apt-get install -y clang libclang-dev }
            "dnf" { sudo dnf install -y clang clang-devel }
            "pacman" { sudo pacman -S --noconfirm clang }
            default { throw "no supported package manager (apt-get/dnf/pacman) found on PATH" }
        }
    }
}

# ── Other native *-sys build inputs seen in this workspace's Cargo.lock ──
# (aws-lc-sys, ring, oxrocksdb-sys, zstd-sys, ...) — not all required by
# every crate, so these are advisory (Required = $false) rather than hard
# blockers; a build will say clearly if one of these is what's missing.
Write-Section "Other native build inputs"

Test-Requirement -Name "cmake" -Required $false -TestScript { Test-CommandExists cmake } `
    -FixDescription "$(if ($OnWindows) {'winget install Kitware.CMake'} elseif ($OnMacOS) {'brew install cmake'} else {'sudo apt-get install -y cmake (or dnf/pacman equivalent)'})" `
    -FixScript {
        if ($OnWindows) {
            if (-not $wingetOk) { throw "winget unavailable — see above" }
            winget install --id Kitware.CMake -e --accept-package-agreements --accept-source-agreements
        } elseif ($OnMacOS) {
            if (-not (Test-CommandExists brew)) { throw "Homebrew not found — install from https://brew.sh first" }
            brew install cmake
        } elseif (Test-CommandExists apt-get) {
            sudo apt-get update; sudo apt-get install -y cmake
        } elseif (Test-CommandExists dnf) {
            sudo dnf install -y cmake
        } elseif (Test-CommandExists pacman) {
            sudo pacman -S --noconfirm cmake
        } else {
            throw "no supported package manager found"
        }
    }

# Detect-only, deliberately no FixScript: this operator prefers not to
# provision Strawberry Perl (its winget installer also needs an interactive
# prompt this script can't answer non-interactively — it self-cancelled
# with exit 1602 the one time this ran unattended). Only a handful of
# *-sys crates hard-require perl specifically (their upstream build.rs
# picks that, not this script) — for any scripting *this workspace* writes
# itself, prefer Rust over reaching for another interpreter at all.
Test-Requirement -Name "perl" -Required $false -TestScript { Test-CommandExists perl } `
    -ManualHint "not auto-installed by this script — only a handful of *-sys crates hard-require it; if perl is genuinely needed, install it by hand"

Test-Requirement -Name "nasm" -Required $false -TestScript { Test-CommandExists nasm } `
    -FixDescription "$(if ($OnWindows) {'winget install NASM.NASM'} elseif ($OnMacOS) {'brew install nasm'} else {'sudo apt-get install -y nasm (or dnf/pacman equivalent)'})" `
    -ManualHint "only needed for optimized asm in some *-sys crates (e.g. aws-lc-sys); builds fall back to a slower path without it" `
    -FixScript {
        if ($OnWindows) {
            if (-not $wingetOk) { throw "winget unavailable — see above" }
            winget install --id NASM.NASM -e --accept-package-agreements --accept-source-agreements
        } elseif ($OnMacOS) {
            if (-not (Test-CommandExists brew)) { throw "Homebrew not found — install from https://brew.sh first" }
            brew install nasm
        } elseif (Test-CommandExists apt-get) {
            sudo apt-get update; sudo apt-get install -y nasm
        } elseif (Test-CommandExists dnf) {
            sudo dnf install -y nasm
        } elseif (Test-CommandExists pacman) {
            sudo pacman -S --noconfirm nasm
        } else {
            throw "no supported package manager found"
        }
    }

Test-Requirement -Name "pkg-config" -Required (-not $OnWindows) -TestScript { Test-CommandExists pkg-config } `
    -FixDescription "$(if ($OnMacOS) {'brew install pkg-config'} elseif (-not $OnWindows) {'sudo apt-get install -y pkg-config (or dnf/pacman equivalent)'} else {'not typically needed on MSVC targets'})" `
    -FixScript {
        if ($OnMacOS) {
            if (-not (Test-CommandExists brew)) { throw "Homebrew not found — install from https://brew.sh first" }
            brew install pkg-config
        } elseif (Test-CommandExists apt-get) {
            sudo apt-get update; sudo apt-get install -y pkg-config
        } elseif (Test-CommandExists dnf) {
            sudo dnf install -y pkgconf-pkg-config
        } elseif (Test-CommandExists pacman) {
            sudo pacman -S --noconfirm pkgconf
        } else {
            throw "no supported package manager found"
        }
    }

# ── Summary ────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════╗"
Write-Host "║  Summary                                          ║"
Write-Host "╚═══════════════════════════════════════════════════╝"
Write-Host "  Target: $Target"
if ($DryRun) {
    Write-Host "  Dry-run — no changes made."
} else {
    Write-Host "  $PASS Fixes applied: $script:FixesApplied"
}
if ($script:StillMissing.Count -gt 0) {
    Write-Host "  $FAIL Still missing (required): $($script:StillMissing -join ', ')" -ForegroundColor Red
    Write-Host ""
    Write-Host "  Re-run this script after resolving the manual hints above."
    exit 1
} else {
    Write-Host "  $PASS All required build dependencies present for target '$Target'."
    exit 0
}
