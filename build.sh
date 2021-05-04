#!/bin/sh

# adapted from: 
# Multi-stage Builds
# 🤓 https://docs.docker.com/develop/develop-images/multistage-build/

# start squid caching http/https proxy
# https://hub.docker.com/r/sameersbn/squid

docker kill -s HUP squid
# docker restart squid 
if [ -z "$(docker ps -q -s -f name=squid)" ] ; then 
  #
  docker pull sameersbn/squid:3.5.27-2
  # or docker build -t sameersbn/squid github.com/sameersbn/docker-squid
  docker run --name squid -d --restart=always \
    --publish 3128:3128 \
    --volume /c0de/_b00t_/docker.🐳/squid.conf:/etc/squid/squid.conf \
    --volume /srv/docker/squid/cache:/var/spool/squid \
    sameersbn/squid:3.5.27-2
fi

# look for a _b00t_ volume
# docker volume create  --mount type=bind,source="$(pwd)"/target,target=/app \


# squid is running (TODO: verify)
# now, we return to the local context: 
#export ftp_proxy=http://172.17.0.1:3128
export http_proxy=http://172.17.0.1:3128
export https_proxy=http://172.17.0.1:3128

echo Building elasticdotventures/b00t:build

# docker build params:
#  --secret stringAray
#  --tag 
#  --platform=<platform>
DOCKER_BUILDKIT=1 \
  sudo docker build \
  --build-arg https_proxy=$https_proxy \
  --build-arg http_proxy=$http_proxy \
  -f Dockerfile \
  .

  # --build-arg "foo"="asdf" \
  #-t b00t -f Dockerfile . 
  # --mount type=bind,source=/c0de/b00t,target=/c0de/b00t \
    
    # --env or --env-file

#sudo DOCKER_BUILDKIT=1 docker build \
#    --build-arg https_proxy=$https_proxy \
#    --build-arg http_proxy=$http_proxy \
#    --mount type=bind,source=/c0de/b00t,target=/c0de/b00t \
#    -t b00t \
#    -f Dockerfile . 

# docker build -t b00t -f Dockerfile .
#docker run -d --name systemd-ubuntu --tmpfs /tmp --tmpfs /run --tmpfs /run/lock  --mount type=bind,source="/c0de",target="/c0de"  --privileged -v /var/run/docker.sock:/var/run/docker.sock -v /sys/fs/cgroup:/sys/fs/cgroup:ro jrei/systemd-ubuntu
exit

docker container create --name extract ???Dunno alexellis2/href-counter:build  
docker container cp extract:/go/src/github.com/alexellis/href-counter/app ./app  
docker container rm -f extract

docker container create --name extract alexellis2/href-counter:build  


docker build --no-cache -t alexellis2/href-counter:latest .
rm ./app
