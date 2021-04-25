#!/bin/bash

## * * * * * * * * * * * \\
#*
#* Ethereum & Solidity
#*
## * * * * * * * * * * * //

# 🪙 CryptoCoin
# 🍰 https://linuxconfig.org/ethereum-mining-on-ubuntu-18-04-and-debian

# safely initialize _b00t_ bash
if [ `type -t "_b00t_init_🥾_开始"` == "function" ]; then 
    # detect _b00t_ environment 
    _b00t_init_🥾_开始
fi

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

