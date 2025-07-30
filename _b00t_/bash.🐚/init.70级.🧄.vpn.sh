#!/bin/bash

## * * * * * * * * * * * \\
#*
#* Tor & VPN
#*
## * * * * * * * * * * * //

# 🧄 Tor & VPN

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "./_b00t_.bashrc"

# https://hub.docker.com/r/dperson/torproxy/
#sudo docker run -it -p 8118:8118 -p 9050:9050 -e TZ=EST5EDT \
#            -d dperson/torproxy

# restrict bandwidth
sudo docker run -it -p 8118:8118 -p 9050:9050 -d dperson/torproxy -b 100
# to adapt default config: 
# sudo docker cp torproxy:/etc/tor/torrc /some/torrc

curl -Lx http://<ipv4_address>:8118 http://jsonip.com/

# smokeping
https://oss.oetiker.ch/smokeping/

# Magic-Wormhole: Get Things From One Computer To Another, Safely
# https://magic-wormhole.readthedocs.io/en/latest/

# bash script to setup a .onion site
#https://github.com/thomasgruebl/darkwebserver
