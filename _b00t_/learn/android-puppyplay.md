---
moto: "Android PuppyPlay Godot game — tap-the-rabbit critter interaction. Built with godot-rust GDExtension 0.2.x (upgraded from early master branch). The BaseCritter moves around screen, detects taps within 50px, emits critter_interacted signal, triggers particles and sounds. BLE treat dispenser integration via RxBlePlugin addon."

build: "just android-test — launches emulator, builds APK, installs, smoke-tests. Pass = Oreo happy 🐶. Fail = crash report. Uses ~/Android/Sdk/emulator with AVD Medium_Phone_API_36.0. Provides screenshot at /tmp/oreo-test.png."

upgrade: "godot-rust upgraded from git master → crates.io 0.2.4. The matured API uses #[derive(GodotClass)], INode2D trait, #[export] for Godot editor properties, #[signal] for GDScript signals, #[func] for callable methods. Builds clean with Rust 1.96."

ble: "RxBlePlugin in addons/ provides BLE access for treat dispenser hardware. Custom Bluetooth stack was built early in the project — research budget consumed here. GDScript rx_android_ble.gd wraps the plugin."

deploy: "adb install over USB or WiFi. android.release.arm64 target in .gdextension config. Cross-compilation needs rustup target add aarch64-linux-android + Android NDK."

crash-guard: "Operator runs `just android-test` before Oreo touches the tablet. Deterministic smoke test: emulator boot → APK install → package check → screenshot. Zero manual device handling. Heartbreak prevention."

# b00t:map v1
# summary: Android PuppyPlay testing workflow — godot-rust GDExtension 0.2.x, emulator smoke test, BLE treat dispenser
# tags: android, godot, rust, gdext, puppyplay, oreo, ble, treat-dispenser, emulator, testing
# tier: ch0nky
# cmds: just android-test, cd puppyplay-godot-droid/src/rust_critters && cargo build --release
# complexity: 6
