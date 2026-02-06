#!/bin/bash

## * * * * * * * * * * * \\
#*
# * 🎬 Video Processing
#*
## blender, ffmpeg
## * * * * * * * * * * * //

## for some fun demos. 
# https://docs.nvidia.com/video-technologies/video-codec-sdk/ffmpeg-with-nvidia-gpu/
# https://hub.docker.com/r/jrottenberg/ffmpeg/
# add -nvidia

# GStreamer
# https://en.wikipedia.org/wiki/GStreamer

# Video Acceleration API 
# https://en.wikipedia.org/wiki/Video_Acceleration_API

# docker pull jrottenberg/ffmpeg
# docker pull jrottenberg/ffmpeg:snapshot-nvidia
# fswebcam -d /dev/video1 --loop 2  blabla.jpg
# ffmpeg -f v4l2 -i video="Integrated Webcam" -f alsa -i hw:0 -profile:v high -pix_fmt yuvj420p -level:v 4.1 -preset ultrafast -tune zerolatency -vcodec libx264 -r 10 -b:v 512k -s 640x360 -acodec aac -strict -2 -ac 2 -ab 32k -ar 44100 -f mpegts -flush_packets 0 udp://192.168.0.108:5000?pkt_size=1316



sudo apt install -y blender


# A collection of Docker containers for running Blender headless or distributed 
# https://github.com/nytimes/rd-blender-docker

## Blender CLI 
## https://docs.blender.org/manual/en/latest/advanced/command_line/arguments.html
# -b --background  run in background
# 🤓 CLI arguments are run in order and position sensitive.
# blender --background test.blend --render-output /tmp --render-frame 1


## CLI Envornment
# BLENDER_USER_CONFIG : Directory for user configuration files.
# BLENDER_USER_SCRIPTS : Directory for user scripts.
# BLENDER_SYSTEM_SCRIPTS: Directory for system wide scripts.
# BLENDER_USER_DATAFILES: Directory for user data files (icons, translations, ..).
# BLENDER_SYSTEM_DATAFILES: Directory for system wide data files.
# BLENDER_SYSTEM_PYTHON:  Directory for system Python libraries.
# TEMP: Store temporary files here.
# TMP: or $TMPDIR Store temporary files here.


# Convert Collada to GLTF
# https://github.com/KhronosGroup/COLLADA2GLTF
git clone --recursive https://github.com/KhronosGroup/COLLADA2GLTF.git
cd COLLADA2GLTF
mkdir build
cd build
cmake .. #-Dtest=ON
# Linux
make
# Windows
## Open the generated COLLADA2GLTF.sln in Visual Studio and build
COLLADA2GLTF-bin --basepath $(pwd) --input $infile --output output/${input}.gltf

# Blender AddOns:
#https://wiki.blender.org/wiki/Process/Addons/Guidelines

## Python in blender
## https://docs.blender.org/api/current/
## https://docs.blender.org/api/current/info_overview.html#:~:text=Python%20in%20Blender,Blender's%20internal%20tools%20as%20well.&text=This%20modifies%20Blender's%20internal%20data%20directly.
# https://docs.blender.org/api/current/info_quickstart.html
## Blender provides its Python modules, such as bpy and mathutils

# https://github.com/nytimes/rd-blender-docker/wiki/Installing-with-GPU-support
docker run --gpus all nytimes/blender:2.82-gpu-ubuntu18.04 nvidia-smi


# Yolo V5: (an AI library for scanning video)
https://github.com/ultralytics/yolov5

# FFmpeg StreamGuide:
https://trac.ffmpeg.org/wiki/StreamingGuide

# Universal Cam project
# PurdueCAM2Project
https://github.com/PurdueCAM2Project/CAM2RetrieveData/wiki/Parsing-EarthCam

# Steps to build an image classifier
https://towardsdatascience.com/all-the-steps-to-build-your-first-image-classifier-with-code-cf244b015799

# Tutorial:
# Deploy a pre-trained image classification model 
# to AZURE FUNCTIONS with PyTorch
https://docs.microsoft.com/en-us/azure/azure-functions/machine-learning-pytorch?tabs=bash

# Streamlink
# A command line utility that extracts streams
# from various services and pipes them into video player of your choice
# https://streamlink.github.io/install.html#windows
# Windows: winget install streamlink
sudo add-apt-repository ppa:nilarimogard/webupd8
sudo apt update
sudo apt-get install -y streamlink

# https://github.com/svt/encore
