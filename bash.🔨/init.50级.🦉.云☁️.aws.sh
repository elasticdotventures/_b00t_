#!/bin/bash

## * * * * * * * * * * * \\
#*
#* AWS is still boilerplate. 
#*
## * * * * * * * * * * * //

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "$_B00T_C0DE_Path/_b00t_.bashrc"

# https://docs.aws.amazon.com/cli/latest/userguide/install-cliv2-docker.html
docker pull amazon/aws-cli:latest

# For amazong linux
#sudo amazon-linux-extras install epel

# S3FS is an s3 "FUSE" filesystem for Linux
# $SUDO_CMD apt-get install -y s3fs
# echo ACCESS_KEY_ID:SECRET_ACCESS_KEY > ${HOME}/.passwd-s3fs
# chmod 600 ${HOME}/.passwd-s3fs
# s3fs growbot.online /mnt/growbot.online -o passwd_file=${HOME}/.passwd-s3fs

