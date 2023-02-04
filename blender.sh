#!/bin/bash

## * * * *
#  Blender in a container
# 🍰 https://github.com/nytimes/rd-blender-docker/wiki/Using-the-Blender-GUI-in-containers

DISPLAY=localhost:10.0
XSOCK=/tmp/.X11-unix
XAUTH=/tmp/.docker.xauth
touch $XAUTH
xauth nlist $DISPLAY | sed -e 's/^..../ffff/' | xauth -f $XAUTH nmerge -

#
# By launching Blender inside the container with --device=/dev/dri/card0:/dev/dri/card0 and --gpus all you should be able to now use it with both Cycles and Eevee.
#
docker run --gpus all -it -v $XSOCK:$XSOCK:rw -v $XAUTH:$XAUTH:rw --device=/dev/dri/card0:/dev/dri/card0 \
	-e DISPLAY=$DISPLAY \
	-e XAUTHORITY=$XAUTH \
        nytimes/blender:2.82-gpu-ubuntu18.04

