# syntax=docker/dockerfile:latest

# TO BUILD:
# ./build.sh 
# TO RUN:


# Docker uses the default 172.17.0.0/16 subnet for container networking. 

# FROM python:3.7-alpine

# shows secret from default secret location:
# RUN --mount=type=secret,id=mysecret cat /run/secrets/mysecret

# USER root 
# SHELL /bin/bash

# 🤓 Dockerfile Best Practices
# https://docs.docker.com/develop/develop-images/dockerfile_best-practices/

# docker CLI syntax
# -f   ::  changes context

# 🤔 Dockerfile can be sent via stdin
# tools like terraform, etc. can generate these
# there is also developer libraries 

# passing ARGS
# An ARG declared before a FROM is outside of a build stage, 
# AND therefore can’t be used in any instruction after a FROM
# ARG outside_build_stage

FROM jrei/systemd-ubuntu as b00t_base
MAINTAINER ops@elastic.ventures
# docker run -d --name systemd-ubuntu --tmpfs /tmp --tmpfs /run --tmpfs /run/lock  --mount type=bind,source="/c0de",target="/c0de"  --privileged -v /var/run/docker.sock:/var/run/docker.sock -v /sys/fs/cgroup:/sys/fs/cgroup:ro jrei/systemd-ubuntu

# Howto setup squid proxy as a sidecar container and have APT use it.
## https://www.serverlab.ca/tutorials/linux/administration-linux/how-to-set-the-proxy-for-apt-for-ubuntu-18-04/
RUN \
if [ -n "$http_proxy" ]; then \
    echo "Acquire { \
  HTTP::proxy \"$http_proxy\"; \
  HTTPS::proxy \"$https_proxy\"; \
}" > /etc/apt/apt.conf.d/http_proxy_b00t_squid;  \
fi 

## NOTE: if squid caching proxy had issue, these lies can cache bad values. 
RUN apt-get clean && apt-get update -y && apt-get upgrade -y
# Timezone
ENV DEBIAN_FRONTEND "noninteractive"
ENV TZ "Australia/Melbourne"
RUN apt-get -y install apt-utils tzdata locales

# Emoji Support
RUN locale-gen en_US.UTF-8
ENV LANG en_US.UTF-8
ENV LANGUAGE en_US:en
ENV LC_ALL en_US.UTF-8

## DOCKER BUILD ENHANCEMENTS
## https://docs.docker.com/develop/develop-images/build_enhancements/
## 
# download github public key
#RUN mkdir -p -m 0600 ~/.ssh && ssh-keyscan github.com >> ~/.ssh/known_hosts
# clone private repo
#RUN --mount=type=ssh git clone git@github.com:myorg/myproject.git myproject
# must run
# $ docker build --ssh default .
# docker --compress

## Only for Dev & Test
RUN apt-get update && apt-get install -y git gcc g++
RUN git --version
RUN apt-get install -y apt-utils dialog curl wget ca-certificates gnupg 

# https://stackoverflow.com/questions/27701930/how-to-add-users-to-docker-container

# TODO: setup ps1, etc. 

#VOLUME "/c0de/_b00t_" 
#COPY ./docker.🐳 /c0de/_b00t_/docker.🐳/

WORKDIR /c0de/_b00t_
ADD ./*.sh  "./"
ADD ./*.bashrc "./"
# ADD /c0de/
RUN chmod +x ./source.sh

## this was screwing up permissions: 
#RUN useradd -ms /bin/bash brianh
#USER brianh
#WORKDIR /home/brianh

## Stage2 
FROM b00t_base as b00t_init
# CURRENT ISSUE: 
# file always rebuilds, full build takes too long,
# not using stages YET
#RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🥾.*.sh"; 
ADD "./*🔨/init.*.🥾.*.sh" "./"
ADD "./_b00t_.bashrc" "./"
RUN chmod +x "_b00t_.bashrc" 
RUN "./_b00t_.bashrc"
#ADD code/requirements.txt /src/requirements.txt
#RUN pip install -r /src/requirements.txt #only re-executed if the file changes
#ADD code /src/code



FROM b00t_init as b00t_init2
## 进口 (Jìnkǒu :: Import/Load) PHASE 2 * * \\ 
# Two is Torvalds Tech (Linux & Git)
RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🐧.*.sh";

FROM b00t_init2 as b00t_init3
## 进口 (Jìnkǒu :: Import/Load) PHASE 2 * * \\ 
# Two is Torvalds Tech (Linux & Git)
RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🐧.*.sh";
RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🐙.*.sh" 


FROM b00t_init3 as b00t_init4
RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🐳.*.sh"

FROM b00t_init4 as b00t_init5
## 进口 (Jìnkǒu :: Import/Load) PHASE 3 * * * \\ 
## minimal c0re Python 🐍
# + establish .venv
RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🐍.*sh"
#RUN source .venv/bin/activate

FROM b00t_init5 as latest
RUN echo $NOW

## Typescript & Node
#RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🚀.*.sh" 
#RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🦄.*.sh" 

## 进口 (Jìnkǒu :: Import/Load) PHASE 4 * * * * \\ 
#RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🤖.*.sh"
#RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.👾.*.sh"
#RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🦉.*.sh"




## arg is an example argument, the exact nature of the syntax is 
# FROM ubuntu as 
#ARG arrrgh
#ARG CODE_VERSION=latest   
#RUN if [ -z "$arrrgh" ] ; then \
#    echo "D0ck3r Starrtup 🐳🏴‍☠️🦜 arrrgh, was not provided"; \
# else \
#    echo "arrrgh 🐳🦜🏴‍☠️📢: $arrrgh /📢"; \
#    fi  \
#    # this example sets up $arrrgh which is an $arrrbitrary value! 
#
#RUN apt update && apt install -y cowsay
#CMD ["/usr/games/cowsay", "Dockerfiles are cool!"]
#

# The VOLUME instruction creates a mount point with the specified 
# name and marks it as holding externally mounted volumes from 
# native host or other containers.
# 🤓 https://docs.docker.com/storage/volumes/
# VOLUME ["/var/www", "/var/log/apache2", "/etc/apache2"]

##* * \\
## yg is a YAML parser/creator like jq (next), it is a cli for .json files
## 🍰 https://github.com/mikefarah/yq
#ENV YQ_VERSION="v4.2.0"  YQ_BINARY="yq_linux_amd64"
#RUN wget https://github.com/mikefarah/yq/releases/download/${VERSION}/${BINARY}.tar.gz -O - |\
# tar xz && mv ${BINARY} /usr/bin/yq
##* * //

##* * \\
## jq is a YAML parser/creator like yq (earlier), it is a cli for .yaml files
## 🍰 https://stedolan.github.io/jq/
#RUN apt-get install jq
##* * //

# Things to copy. 
# ADD [--chown=<user>:<group>] <src>... <dest>


#ENTRYPOINT [ "executable" ]
##🤓 snapshot/layer explained: 
## when FROM is executed, 
## files can be eliminated from forward repositories when
# they are no longer needed.  
# This approach hardens the application by removing dependencies. 
# step1.) make a branch in the code: git branch alpha_v1
# step2.) delete the files no longer needed (i.e. keyVaults)
## Svelte Phase

# To enable ssh & remote debugging on app service change the base image to the one below
# FROM mcr.microsoft.com/azure-functions/python:3.0-python3.8-appservice

#FROM base AS live-branch-v1
#RUN echo "live Branch sets Is_EnvLive=1"
#ENV _Env_Is="live" \
#    Is_EnvDev=0    \
#    Is_EnvTest=0   \
#    Is_EnvLive=1   

#FROM base AS dev-branch-v1
#RUN echo "dev Branch sets Is_EnvDev=1"
#ENV _Env_Is="dev" \
#    Is_EnvDev=1   \
#    Is_EnvTest=0  \
#    Is_EnvLive=0  

#FROM base AS test-branch-v1
#RUN echo "test Branch sets Is_EnvTest=1"
#ENV _Env_Is="test"
#ENV Is_EnvDev=1   \
#    Is_EnvTest=0  \
#    Is_EnvLive=0  


# apparently LABEL is documented in docs, but not actually supported.
# LABEL description="experimental sportsworldas2 instance"




## Google Cloud SDK
## https://cloud.google.com/sdk/docs/install#deb
#RUN echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] http://packages.cloud.google.com/apt cloud-sdk main" | tee -a /etc/apt/sources.list.d/google-cloud-sdk.list && curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | apt-key --keyring /usr/share/keyrings/cloud.google.gpg  add - && apt-get update -y && apt-get install google-cloud-sdk -y

## Database! 
#RUN apt-get install mariadb-server python-mysqldb
#COPY mariadb/mysqld.cnf /etc/mysql/conf.d/
#RUN mkdir -p /mnt/mariadb-datadir/data
#RUN mkdir -p /mnt/mariadb-datadir/logs
# RUN sudo mysql_secure_installation

# https://hub.docker.com/_/mariadb
# RUN apt-get install -y libmysqlclient-dev

## attempt 1: .. untested!
# RUN docker pull mariadb/server:latest
## attempt 2: runc .. untested!
## RUN docker run --runtime=sysbox-runc -it some-image
# apt-get install -y runc

## attempt 3: kata container .. untested!
#RUN sudo apt install snapd
#RUN sudo snap install kata-containers --classic
## attempt 3b: https://github.com/kata-containers/documentation/blob/master/install/ubuntu-installation-guide.md
#ENV ARCH=$(arch)
#ENV BRANCH="${BRANCH:-master}"
#RUN sudo sh -c "echo 'deb http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/${BRANCH}/xUbuntu_$(lsb_release -rs)/ /' > /etc/apt/sources.list.d/kata-containers.list"
#RUN curl -sL  http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/${BRANCH}/xUbuntu_$(lsb_release -rs)/Release.key | sudo apt-key add -
#RUN sudo -E apt-get update
#RUN sudo -E apt-get -y install kata-runtime kata-proxy kata-shim

## 3b: install docker
# https://github.com/kata-containers/documentation/blob/master/install/docker/ubuntu-docker-install.md
#RUN sudo -E apt-get -y install apt-transport-https ca-certificates software-properties-common
#RUN curl -sL https://download.docker.com/linux/ubuntu/gpg | sudo apt-key add -
#RUN arch=$(dpkg --print-architecture)
#RUN sudo -E add-apt-repository "deb [arch=${arch}] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable"
#RUN sudo -E apt-get update
#RUN sudo -E apt-get -y install docker-ce
# RUN systemctl start docker

#. /etc/os-release
#sudo sh -c "echo 'deb http://download.opensuse.org/repositories/devel:/kubic:/libcontainers:/stable/xUbuntu_${VERSION_ID}/ /' > /etc/apt/sources.list.d/devel:kubic:libcontainers:stable.list"
#wget -nv https://download.opensuse.org/repositories/devel:kubic:libcontainers:stable/xUbuntu_${VERSION_ID}/Release.key -O Release.key
#sudo apt-key add - < Release.key
#RUN apt-get -y update
#RUN apt-get -qq -y install buildah podman
#``



############ PODMAN & BUILDAH


### https://computingforgeeks.com/how-to-install-podman-on-debian/
## step1: update system
# RUN apt -y update && apt -y upgrade
## step2: install pre-reqs
#RUN apt -y install \
#  gcc \
#  make \
#  cmake \
#  git \
#  btrfs-progs \
#  golang-go \
#  go-md2man \
#  iptables \
#  libassuan-dev \
#  libc6-dev \
#  libdevmapper-dev \
#  libglib2.0-dev \
#  libgpgme-dev \
#  libgpg-error-dev \
#  libostree-dev \
#  libprotobuf-dev \
#  libprotobuf-c-dev \
#  libseccomp-dev \
#  libselinux1-dev \
#  libsystemd-dev \
#  pkg-config \
#  runc \
#  uidmap \
#  libapparmor-dev

## step2B: install/upgrade go
# https://www.vultr.com/docs/install-the-latest-version-of-golang-on-ubuntu
#WORKDIR /tmp
#RUN wget https://golang.org/dl/go1.16.3.linux-amd64.tar.gz
#RUN tar -C /usr/local -xzf go1.16.3.linux-amd64.tar.gz
#RUN echo "export PATH=$PATH:/usr/local/go/bin" >> ~/.profile
#RUN echo "export GOPATH=~/.go" >> ~/.profile
#RUN /bin/sh ~/.profile
#RUN rm -f /usr/bin/go
#RUN ln -s /usr/local/go/bin/go /usr/bin/go

## step3:  conmon & podman
#WORKDIR /tmp
#RUN git clone https://github.com/containers/conmon
#WORKDIR /tmp/conmon
#RUN /usr/bin/make
#RUN /usr/bin/make podman
#RUN cp /usr/local/libexec/podman/conmon  /usr/local/bin/
#RUN rm -Rf /tmp/conmon

# Step4:  install CNI plugins
#WORKDIR /tmp
#RUN git clone https://github.com/containernetworking/plugins.git /tmp/src/github.com/containernetworking/plugins
#WORKDIR /tmp/src/github.com/containernetworking/plugins
#RUN ./build_linux.sh
#RUN mkdir -p /usr/libexec/cni
#RUN cp bin/* /usr/libexec/cni

# step5: setup CNI networking
#RUN mkdir -p /etc/cni/net.d
#RUN curl -qsSL https://raw.githubusercontent.com/containers/libpod/master/cni/87-podman-bridge.conflist | tee /etc/cni/net.d/99-loopback.conf

# step6: populate configuration files
#RUN mkdir -p /etc/containers
#RUN curl https://raw.githubusercontent.com/projectatomic/registries/master/registries.fedora -o /etc/containers/registries.conf
#RUN curl https://raw.githubusercontent.com/containers/skopeo/master/default-policy.json -o /etc/containers/policy.json

# step7: install podman
#WORKDIR /tmp
#RUN git clone https://github.com/containers/libpod/ /tmp/src/github.com/containers/libpod
#WORKDIR /tmp/src/github.com/containers/libpod
#RUN make install

## PODMAN FINAL RESULT: 
# $ podman run -dt -p 3306:3306/tcp docker.io/mariadb:latest
# ERRO[0000] Failed to built-in GetDriver graph btrfs /var/lib/containers/storage
# ERRO[0000] Error loading CNI config file /etc/cni/net.d/99-loopback.conf: error parsing configuration: missing 'type'
# ERRO[0013] unable to write volume event: "write unixgram @00355->/run/systemd/journal/socket: sendmsg: no such file or directory"
# ERRO[0013] unable to write pod event: "write unixgram @00355->/run/systemd/journal/socket: sendmsg: no such file or directory"
# ERRO[0013] Error preparing container 67c8a93691c0950d816b1950ec6a0dfd747d94b16ee4f4aebe2c2694909e2bcb: error creating network namespace for container 67c8a93691c0950d816b1950ec6a0dfd747d94b16ee4f4aebe2c2694909e2bcb: mount --make-rshared /run/netns failed: "operation not permitted"
# Error: failed to mount shm tmpfs "/var/lib/containers/storage/vfs-containers/67c8a93691c0950d816b1950ec6a0dfd747d94b16ee4f4aebe2c2694909e2bcb/userdata/shm": operation not permitted


# usermod -aG docker ${USER}


# RUN docker run --name sportsworld-as2-mdb \
#    -v /mnt/mariadb-datadir/data:/var/lib/mysql \
#    -e MYSQL_ROOT_PASSWORD="Sp{{0}}rtsw{{0}}rld" \
#    -d mariadb:latest

# COPY utils/wait-for-it.sh /wait-for-it.sh
# RUN pip install 

# Install pip requirements
#RUN apt-get install -y gunicorn
#RUN apt-get install -y python-gevent
# RUN python -m pip install gunicorn



## django dir
#COPY ./django /home/site/wwwroot/

## NOTE: at this point django/* is in /home/site/wwwroot
## it's MOVED UP a level in the tree so ./wwwroot == ./django
#WORKDIR /home/site/
#RUN pip install -r ./wwwroot/requirements.txt

# move back to wwwroot so djapp.wsgi can be found! 
#WORKDIR /home/site/wwwroot
#RUN touch ./docker_build_time.txt

# During debugging, this entry point will be overridden. For more information, please refer to https://aka.ms/vscode-docker-python-debug
# File wsgi.py was not found in subfolder: 'sportsworld-as2'. Please enter the Python path to wsgi file.
# CMD ["/bin/sh", "-c", "gunicorn", "--bind", "0.0.0.0:8000", "djapp.wsgi"]
# CMD [ "gunicorn", "--bind", "0.0.0.0:8000", "djapp.wsgi"]
# COPY ./startup.sh /
# RUN chmod +x /startup.sh

# CMD [ "/bin/sh", "-c", "/home/site/wwwroot/startup.sh" ]



