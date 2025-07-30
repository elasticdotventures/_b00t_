#!/bin/bash

## * * * * * * * * * * * \\
#*
# * 📱 Mobile
#*
## * * * * * * * * * * * //

# https://mirrors.tuna.tsinghua.edu.cn/

## ReDroid / Android Emulator
# start and connect via `scrcpy` (Performance boost, *recommended*)
docker run -itd --rm --memory-swappiness=0 --privileged -v ~/data:/data -p 5555:5555 redroid/redroid:10.0.0-latest

sudo apt-get install -y adb scrcpy
sudo apt install 

adb connect localhost:5555:5555
scrcpy --serial <IP>:5555

# List of useful iOS & Swift stuff!!
# https://github.com/jphong1111/Useful_Swift