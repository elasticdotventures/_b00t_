#!/bin/bash

# TODO: /etc/sshd/sshd_config
X11Forwarding yes
X11DisplayOffset 10
# set default text editor for text/plain

# ~/.ssh/config
# ssh -o ForwardX11=yes
ForwardX11 yes

xdg-mime default code.desktop text/plain
#sudo update-alternatives --set editor /usr/bin/code
#sudo update-alternatives --install editor /usr/bin/editor $(which code)
sudo apt-get install sux

## MOVED: in _b00t_.bashrc
# export DISPLAY=":10"
