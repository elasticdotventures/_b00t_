#!/bin/bash

## * * * * * * * * * * * \\
#*
#* Ethereum & Solidity
#*
## * * * * * * * * * * * //

# 🥯 CryptoCoin
# 🍰 https://linuxconfig.org/ethereum-mining-on-ubuntu-18-04-and-debian

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "./_b00t_.bashrc"


sudo add-apt-repository ppa:ethereum/ethereum
sudo apt update

sudo apt install ethereum

$ mkdir ethminer
$ wget -O ethminer/ethminer.tar.gz https://github.com/ethereum-mining/ethminer/releases/download/v0.18.0/ethminer-0.18.0-cuda-9-linux-x86_64.tar.gz
$ tar xzf ethminer/ethminer.tar.gz -C ethminer/
$ ethminer/bin/ethminer --help
ethminer 0.18.0
Build: linux/release/gnu

Ethminer - GPU ethash miner
minimal usage : ethminer [DEVICES_TYPE] [OPTIONS] -P... [-P...]


# ChiaCoin
# Checkout the source and install
# git clone https://github.com/Chia-Network/chia-blockchain.git -b latest --recurse-submodules
# cd chia-blockchain
# chmod +x ./install.sh
# ./install.sh

. ./activate

# for electron under ubuntu
apt-get install -y libgtkextra-dev libgconf2-dev libnss3 libasound2 libxtst-dev
apt-get install -y libatk-bridge-2.0-dev 
apt-get install -y libgtk-3-0-0 libgtk-3.0-dev
npm install electron -g
# TO RUN:
# chia/chia-blockhain/chia-blockchain-gui
# 