# 🪟  Build b00t Windows desktop host with native tray.
#     Run from WSL. Requires: cross or direct Windows build.
#     Output: vendor/ledgrrr/target/x86_64-pc-windows-msvc/release/ledgerr-tauri.exe
build-windows-tray:
    cross build --release --target x86_64-pc-windows-msvc -p ledgerr-tauri
    @echo "✅ Windows binary: vendor/ledgrrr/target/x86_64-pc-windows-msvc/release/ledgerr-tauri.exe"
    @echo "   Copy to Windows and run: ledgerr-tauri.exe"
