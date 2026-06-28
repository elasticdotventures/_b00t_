---
moto: "MagiskSSH — SSH server for rooted Android devices, installed as a Magisk module. Enables remote shell access to Android tablets/phones via standard SSH. 145 commits, GPLv3, maintained since 2018."

use-case: "For Oreo's app4dog: once installed on the Android tablet, b00t can SSH in remotely to install APKs, run adb commands, control the BLE treat dispenser, capture screenshots, and monitor app crashes — all without physical access to the device. Combined with just android-test, this completes the remote testing pipeline."

setup: "Install Magisk on rooted Android device → flash MagiskSSH module → configure SSH keys → b00t ssh user@tablet 'adb install app.apk'. For non-rooted devices, adb over TCP/IP (adb tcpip 5555) is a lighter alternative."

integration: "The RHAI script _b00t_/scripts/android-emu-setup.rhai uses emulator today. With MagiskSSH, the same script can target a physical device: replace 'emulator -avd' with 'ssh tablet' and skip the boot wait. Physical device testing is more reliable than emulator for BLE hardware (treat dispenser needs real Bluetooth)."

# b00t:map v1
# summary: MagiskSSH — SSH server for rooted Android (Magisk module, GPLv3, remote device access)
# tags: magisk, ssh, android, remote, tablet, device-management, rooting, adb
# tier: ch0nky
# cmds: ssh user@tablet, adb connect tablet:5555
# complexity: 4
