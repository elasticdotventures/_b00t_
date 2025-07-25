# 1. Download the Google signing key
wget -q -O - https://dl.google.com/linux/linux_signing_key.pub | sudo gpg --dearmor -o /usr/share/keyrings/google-linux.gpg

# 2. Add the Google Chrome repository
echo 'deb [arch=amd64 signed-by=/usr/share/keyrings/google-linux.gpg] https://dl.google.com/linux/chrome/deb/ stable main' | sudo tee /etc/apt/sources.list.d/google-chrome.list

# 3. Update APT
sudo apt update

# 4. Install Chrome
sudo apt install google-chrome-stable

