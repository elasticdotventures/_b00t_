#!/bin/bash

source "$_B00T_C0DE_Path/_b00t_.bashrc"


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

# 🤓 nvidia-smi is an awesome tool for figuring out if cude is installed properly
# sudo docker run --rm --gpus all nvidia/cuda:11.0-base nvidia-smi

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
# NVIDA Version
cat /proc/driver/nvidia/version

## 🤓 sometimes a reboot helps here! 

## setting up a render node w/docker
## WRONG, no longer supported by nvidia: 
## 🍰 https://snowgoons.ro/posts/2020-09-08-setting-up-a-blender-rendering-node-using-docker/
curl -s -L https://nvidia.github.io/nvidia-container-runtime/gpgkey | \
  sudo apt-key add -
distribution=$(. /etc/os-release;echo $ID$VERSION_ID)
curl -s -L https://nvidia.github.io/nvidia-container-runtime/$distribution/nvidia-container-runtime.list | \
  sudo tee /etc/apt/sources.list.d/nvidia-container-runtime.list
# NOTE:  step is UNNECESSARY, kept for historical reminder. 
#curl -s -L https://nvidia.github.io/nvidia-docker/$distribution/nvidia-docker.list | \
#  sudo tee /etc/apt/sources.list.d/nvidia-docker.list

sudo apt-get update
sudo apt-get install -y nvidia-container-runtime
sudo apt-get install -y nvidia-docker2

# https://github.com/NVIDIA/nvidia-docker
# docker run nvidia/cuda nvidia-smi
# https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/install-guide.html#install-guide
# sudo apt-get install -y nvidia-docker2
sudo systemctl restart docker


# List of Jupyter Notebooks: 
# https://jupyter-docker-stacks.readthedocs.io/en/latest/using/selecting.html
# jupyter/base-notebook
# jupyter/minimal-notebook
# jupyter/scipy-notebook
# jupyter/tensorflow-notebook


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

# Anime Gan2
# https://github.com/bryandlee/animegan2-pytorch
# Anime to sketch
# https://github.com/Mukosame/Anime2Sketch

# AIZythFinder Retrosynthetic Planning 
# https://molecularai.github.io/aizynthfinder/gui.html


pip install mmcv-full -f https://download.openmmlab.com/mmcv/dist/cu110/torch1.7.0/index.html



# MMEdit: image inpainting, 
# https://github.com/open-mmlab/mmediting/blob/master/docs/install.md
git clone https://github.com/open-mmlab/mmediting
docker build -t mmediting docker/
docker run --gpus all --shm-size=8g -it -v {DATA_DIR}:/mmediting/data mmediting

# Command-Line tools for speech and intent recognition on Linux 
# https://voice2json.org/

