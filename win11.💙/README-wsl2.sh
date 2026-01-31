#!/bin/sh
LINUX_IP=$(ip addr | awk '/inet / && !/127.0.0.1/ {split($2,a,"/"); print a[1]}')
WINDOWS_IP=$(ip route | awk '/^default/ {print $3}')
# Elevate to administrator status then run netsh to add firewall rule
powershell.exe -Command "Start-Process netsh.exe -ArgumentList \"advfirewall firewall add rule name=X11-Forwarding dir=in action=allow program=%ProgramFiles%\VcXsrv\vcxsrv.exe localip=$WINDOWS_IP remoteip=$LINUX_IP localport=6000 protocol=tcp\" -Verb RunAs"

# credit https://thomasward.com/wsl2-x11/
