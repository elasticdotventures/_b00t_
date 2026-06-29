---
aohp: "AOHP — Android Open Harness Project. AOSP-based OS-level agent harness treating AI agents as first-class OS actors. Agents perceive screen, plan actions, execute via input injection, and verify results at the OS level. Reported: +21% task completion, −52% token cost."

relevance: "For app4dog tablet testing: AOHP eliminates renderer dependency. Instead of waiting for Godot to fix Mali-G57 input, b00t uses AOHP to perceive the tablet screen, inject taps, and verify game state. OS-level agent bypasses app renderer bugs."

capability: "AOHP provides: screen perception (screenshot → state), action planning (what to tap), input injection (adb shell input), and result verification (screenshot comparison). This is the missing layer between b00t's QA test scripts and the tablet's broken renderer."

integration: "b00t + AOHP pipeline: b00t QA script → AOHP perceives screen → plans next tap → injects input → verifies result → reports back. The QA script doesn't care about renderer bugs — AOHP handles the OS-level interaction."

# b00t:map v1
# summary: AOHP — AOSP-based OS-level agent harness for Android tablet testing. Bypasses app renderer bugs.
# tags: aohp, android, aosp, agent, os-level, testing, tablet, screen-perception
# tier: ch0nky
# cmds: adb shell screencap, adb shell input tap, AOHP agent perceive → plan → act → verify
# complexity: 6
