#!/bin/bash

## * * * * * * * * * * * \\
#*
#* AI 
#*
## * * * * * * * * * * * //

# NVIDA CUDA is notorious for IOWOMM (It only works on my machine)
# cuda didn't install? something odd, try these commands 
#    sudo apt clean; sudo apt update; 
#    sudo apt purge cuda; 
#    sudo apt purge nvidia-*; 
#    sudo apt autoremove; sudo apt install cuda

# the default CUDA versions are always too old, so commands below re-install them! 
#sudo apt install -y nvidia-driver-450
tmpdir=$(mktemp -d)
cd $tmpdir
if [ ! checkOS ] ; then 
    # skip cuda. it'll go to hell
elif [ is_WSLv2_🐧💙🪟v2 ] ; then 
    ## WSL2
    wget https://developer.download.nvidia.com/compute/cuda/repos/wsl-ubuntu/x86_64/cuda-wsl-ubuntu.pin
    sudo mv cuda-wsl-ubuntu.pin /etc/apt/preferences.d/cuda-repository-pin-600

    sudo apt-key adv --fetch-keys https://developer.download.nvidia.com/compute/cuda/repos/wsl-ubuntu/x86_64/7fa2af80.pub
    sudo add-apt-repository "deb https://developer.download.nvidia.com/compute/cuda/repos/wsl-ubuntu/x86_64/ /"
    sudo apt-get update
    sudo apt-get -y install cuda

# verify checksums?
# https://developer.download.nvidia.com/compute/cuda/11.3.0/docs/sidebar/md5sum.txt
else
    ## NOT WSL2
    wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2004/x86_64/cuda-ubuntu2004.pin
    sudo mv cuda-ubuntu2004.pin /etc/apt/preferences.d/cuda-repository-pin-600

    sudo apt-key adv --fetch-keys https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2004/x86_64/7fa2af80.pub
    sudo add-apt-repository "deb https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2004/x86_64/ /"
    sudo apt-get update
    sudo apt-get -y install cuda

fi
## * * * *
## installed aptitude







# 🧠 AI
# Game Agent Framework:
# https://github.com/SerpentAI/SerpentAI

# Yolo V5:
# In Video 
# First order motion:
# https://github.com/AliaksandrSiarohin/first-order-model
# Checkpoints:
# https://drive.google.com/drive/folders/1PyQJmkdCsAkOYwUyaj_l-l0as-iLDgeH

# Jupyter notebook data-science stack
# REPO https://github.com/jupyter/docker-stacks
# DOCS: https://jupyter-docker-stacks.readthedocs.io/en/latest/index.html
# docker pull jupyter/datascience-notebook

