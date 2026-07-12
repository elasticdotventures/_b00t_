# servo.just — Servo browser engine recipes
# Import: mod servo '_b00t_/servo.just'
#
# 🤓 Servo uses WebDriver (W3C), NOT Chrome DevTools Protocol (CDP)
#    For CDP-based RPA use qutebrowser --remote-debugging-port or chrome-headless
# 🤓 Always use podman (not docker) — rootless, no daemon

IMAGE  := "localhost/b00t-servo:latest"
CFILE  := "_b00t_/Containerfile.servo"

# Build the servo container image
build:
    podman build -f {{ CFILE }} -t {{ IMAGE }} .
    @echo "built {{ IMAGE }}"

# Browse a URL headlessly (one-shot, prints to stdout via --dump-style or just runs)
browse url="https://servo.org":
    podman run --rm -it \
      -e DISPLAY=:99 \
      {{ IMAGE }} \
      xvfb-run --server-args="-screen 0 1280x1024x24" \
      servo --no-native-titlebar "{{ url }}"

# Start WebDriver server on :4444 (detached) — connect via Selenium/Playwright
# Usage: just servo::webdriver; python3 -c "from selenium import webdriver; ..."
webdriver port="4444":
    podman run --rm -d \
      --name servo-webdriver \
      -p {{ port }}:4444 \
      {{ IMAGE }} \
      xvfb-run --server-args="-screen 0 1280x1024x24" \
      servo --webdriver={{ port }} about:blank
    @echo "WebDriver listening on localhost:{{ port }}"
    @echo "Stop with: podman stop servo-webdriver"

# Stop running WebDriver container
webdriver-stop:
    podman stop servo-webdriver 2>/dev/null || true

# Open Firefox DevTools connection (debug protocol, NOT CDP)
devtools url="https://servo.org" port="6080":
    podman run --rm -d \
      --name servo-devtools \
      -p {{ port }}:6080 \
      {{ IMAGE }} \
      xvfb-run --server-args="-screen 0 1280x1024x24" \
      servo --devtools={{ port }} "{{ url }}"
    @echo "Firefox DevTools on localhost:{{ port }}"

# Render URL to PNG screenshot (requires --screenshot flag support)
screenshot url="https://servo.org" out="/tmp/servo-screenshot.png":
    podman run --rm \
      -v /tmp:/tmp:z \
      {{ IMAGE }} \
      xvfb-run --server-args="-screen 0 1280x1024x24" \
      servo --screenshot "{{ out }}" "{{ url }}"
    @echo "Screenshot → {{ out }}"

# Install servo binary directly (no container — requires runtime deps via apt)
install:
    b00t-cli cli install servo

# Pull the servo image (skip build if already available)
pull:
    podman pull {{ IMAGE }} 2>/dev/null || just servo::build

# Show servo version (container)
version:
    podman run --rm {{ IMAGE }} servo --version
