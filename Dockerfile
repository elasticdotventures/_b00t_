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
# 🤓 Buildkit syntax 
# https://github.com/moby/buildkit/blob/master/frontend/dockerfile/docs/syntax.md

# docker CLI syntax
# -f   ::  changes context

# 🤔 Dockerfile can be sent via stdin
# tools like terraform, etc. can generate these
# there is also developer libraries 

# passing ARGS
# An ARG declared before a FROM is outside of a build stage, 
# AND therefore can’t be used in any instruction after a FROM
# ARG outside_build_stage






#### 
# Step1: init
FROM jrei/systemd-ubuntu as b00t_1n1t
LABEL 🥾🐳 b00t_1n1t 
ARG arrgh 
ENV "STAGE"="1n1t"
RUN echo "🥾🐳 1n1t"
RUN echo "STAGE: ${STAGE} arrgh: ${arrgh}"


## make logs persistent 
VOLUME ["/var/log" ]


# Howto setup squid proxy as a sidecar container and have APT use it.
## https://www.serverlab.ca/tutorials/linux/administration-linux/how-to-set-the-proxy-for-apt-for-ubuntu-18-04/
RUN \
if [ -n "$http_proxy" ]; then \
    echo "Acquire { \
  HTTP::proxy \"$http_proxy\"; \
  HTTPS::proxy \"$https_proxy\"; \
}" > /etc/apt/apt.conf.d/http_proxy_b00t_squid;  \
fi 
RUN echo "apt update -y && apt upgrade -y && apt-get install -y apt-utils"

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

#### 
# Step2: base (everything)
FROM b00t_1n1t as b00t_b4s3
LABEL 🥾🐳 B4S3 

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

## Dev/test git, gcc, g++
RUN --mount=type=cache,target=/var/cache/apt \
    --mount=type=cache,target=/var/lib/apt \
    apt update && apt-get --no-install-recommends install -y apt-utils git gcc g++
#RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
#  apt update && apt-get --no-install-recommends install -y gcc

# RUN apt-get update && apt-get install -y git gcc g++
RUN git --version
RUN apt-get install -y apt-utils dialog curl wget ca-certificates gnupg 

# https://stackoverflow.com/questions/27701930/how-to-add-users-to-docker-container
#RUN gosu groupadd docker
#RUN useradd --create-home --gid docker brianh

# TODO: setup ps1, etc. 
FROM b00t_b4s3  as b00t_m4k3
LABEL 🥾🐳 M4K3


#VOLUME "/c0de/_b00t_" 
#COPY ./docker.🐳 /c0de/_b00t_/docker.🐳/
#VOLUME "/c0de/_b00t_"
WORKDIR /c0de/_b00t_/
ADD ./*.sh  "./"
ADD ./*.bashrc "./"
# ADD /c0de/
RUN chmod +x ./source.sh

## this was screwing up permissions: 
#RUN useradd -ms /bin/bash brianh
#USER brianh
#WORKDIR /home/brianh




## Stage2 
#FROM b00t_m4k3 as b00t_latest
# CURRENT ISSUE: 
# file always rebuilds, full build takes too long,
# not using stages YET
#RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🥾.*.sh"; 
ADD "./_b00t_.bashrc" "./"
ADD "./source.sh" "./"
RUN --mount=type=bind,target="/c0de/_b00t_",ro \
 ./source.sh ./bash.🔨/init.*.🥾.*.sh


RUN --mount=type=bind,target="/c0de/_b00t_",ro \
 source ./_b00t_.bashrc; \
 if [ -z "$arrgh" ] ; then \
  echo "D0ck3r Starrtup 🐳🏴‍☠️🦜 arrrgh, was not provided"; \
 else \
    echo "arrrgh 🐳🦜🏴‍☠️📢: $arrrgh /📢"; \
 fi 
 # this example sets up $arrrgh which is an $arrrbitrary value! 

#RUN apt update && apt install -y cowsay
#CMD ["/usr/games/cowsay", "Dockerfiles are cool!"]

## 进口 (Jìnkǒu :: Import/Load) PHASE 2 * * \\ 
# Two is Torvalds Tech (Linux & Git)
#ADD "./*🔨/init.*.🐧.*.sh" "./"
RUN --mount=type=bind,target="/c0de/_b00t_",ro \
 ./source.sh "./bash.🔨/init.*.🐧.*.sh";


#ADD "./*🔨/init.*.🐙.*.sh" "./"
RUN  --mount=type=bind,target="/c0de/_b00t_",ro \
 ./source.sh  "./bash.🔨/init.*.🐙.*.sh" 

RUN  --mount=type=bind,target="/c0de/_b00t_",ro \
./source.sh "./bash.🔨/init.*.🐳.*.sh"

## 进口 (Jìnkǒu :: Import/Load) PHASE 3 * * * \\ 

## minimal c0re Python 🐍
# + establish .venv
RUN  --mount=type=bind,target="/c0de/_b00t_",ro \
./source.sh "./bash.🔨/init.*.🐍.*sh"
#RUN source .venv/bin/activate

## Typescript & Node
#RUN  --mount=type=bind,target="/c0de/_b00t_",ro \
# ./source.sh "./bash.🔨/init.*.🚀.*.sh" 
#RUN  --mount=type=bind,target="/c0de/_b00t_",ro \
# ./source.sh "./bash.🔨/init.*.🦄.*.sh" 

# future: java & go
# Use files from an external image! 
# COPY --from=nginx:latest /etc/nginx/nginx.conf /nginx.conf

#RUN ln -sf /c0de/_b00t_/_b00t_.bashrc 
RUN echo "😁" >build.done

## 进口 (Jìnkǒu :: Import/Load) PHASE 4 * * * * \\ 
#RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🤖.*.sh"
#RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.👾.*.sh"
#RUN /c0de/_b00t_/source.sh "./bash.🔨/init.*.🦉.*.sh"
# CMD [ ".//sbin/entrypoint.sh" ]

ENTRYPOINT ["bash"]
CMD ["--rcfile","./_b00t_.bashrc"]

## 
ENTRYPOINT ["tail"]
CMD ["-f","/dev/null"]
## docker run -d elasticdotventures/b00t tail -f /dev/null
##


#CMD [ "/bin/bash", "-c", "/c0de/_b00t_/_b00t_.bashrc"]
#RUN python -m venv /venv
#ENV PATH=/venv/bin:$PATH
