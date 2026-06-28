---
moto: "Android Godot game with Rust GDExtension — tap-to-interact critter pattern. Built with godot-rust 0.2.x (stable crates.io release). BaseCritter (Node2D) moves around screen, detects taps within configurable radius, emits interaction signal, triggers particles and sounds. BLE peripheral integration via RxBlePlugin addon for hardware control."

build: "Hermetic container build: ubuntu:22.04 + NDK 27 + SDK 35 + Godot 4.4.1 + export templates. Cross-compiles aarch64 .so (3.4MB stripped). Gradle configuration cache + build cache for incremental rebuilds. just android-container-build for deterministic APK production."

upgrade: "godot-rust upgraded from git master → crates.io 0.2.4. The matured API uses #[derive(GodotClass)], INode2D trait, #[export] for Godot editor properties, #[signal] for GDScript signals, #[func] for callable methods. Builds clean with Rust 1.96."

ble: "RxBlePlugin in addons/ provides BLE access for peripheral hardware. GDScript rx_android_ble.gd wraps the plugin with connect, scan, and write characteristic methods. Custom GATT service UUIDs configurable per hardware device."

deploy: "adb install over USB or WiFi. android.release.arm64 target in .gdextension config. Cross-compilation needs aarch64-linux-android target + Android NDK. Container build handles all toolchain deps."

test: "RHAI emulator pipeline: deps→boot→build→install→screenshot. Deterministic smoke test via android-sandbox recipe. MagiskSSH for remote device management without physical access."

# b00t:map v1
# summary: Android Godot Rust pipeline — GDExtension 0.2.x, hermetic container build, emulator smoke test, BLE peripheral integration
# tags: android, godot, rust, gdext, container, emulator, ble, ndk, cross-compile
# tier: ch0nky
# cmds: just android-container-build, just android-sandbox, cargo build --target aarch64-linux-android --release
# complexity: 6
