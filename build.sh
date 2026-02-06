#!/bin/bash

# https://nvie.com/posts/a-successful-git-branching-model/
# master, develop, release-*, hotfix-*

# steps to git merge
# https://stackoverflow.com/questions/56577184/github-pull-changes-from-a-template-repository/56577320
# git remote add template https://github.com/elasticdotventures/_b00t_.git
# git fetch --all
# git merge template/develop --allow-unrelated-histories

# adapted from: 
# Multi-stage Builds
# 🤓 https://docs.docker.com/develop/develop-images/multistage-build/

# start squid caching http/https proxy
# https://hub.docker.com/r/sameersbn/squid

source "./_b00t_.bashrc"

docker kill -s HUP squid
# docker restart squid 
if [ -z "$(docker ps -q -s -f name=squid)" ] ; then 
  #
  log_📢_记录 🦑 installing local squid container for build cache
  docker pull sameersbn/squid:3.5.27-2
  # or docker build -t sameersbn/squid github.com/sameersbn/docker-squid
  docker run --name squid -d --restart=always \
    --publish 3128:3128 \
    --volume /c0de/_b00t_/docker.🐳/squid.conf:/etc/squid/squid.conf \
    --volume /srv/docker/squid/cache:/var/spool/squid \
    sameersbn/squid:3.5.27-2
fi

./test/bats/bin/bats test/test.bats

@test "can run our script" {
    ./project.sh
}
# look for a _b00t_ volume
# docker volume create  --mount type=bind,source="$(pwd)"/target,target=/app \


# squid is running (TODO: verify)
# now, we return to the local context: 
#export ftp_proxy=http://172.17.0.1:3128

log_📢_记录 🦑 setting http proxy
export http_proxy=http://172.17.0.1:3128
export https_proxy=http://172.17.0.1:3128

echo Building elasticdotventures/b00t:build

TARGET="b00t_m4k3"  # future, config point. 
#fzmenu \
#    new
#    sandbox: "问题"
#    make: "问题"
#    edit: "问题"
TARGET="fr3sh"
# FUTURE: consent point

# docker build params:
#  --secret stringAray
#  --tag 
#  --platform=<platform>
echo "TARGET: $TARGET"
  # --build-arg BUILDKIT_INLINE_CACHE=1 .
  # --progress=plain \
  #--build-arg arrgh="🦜🏴‍☠️" \

log_📢_记录 
export DOCKER_BUILDKIT=1

docker buildx install
docker buildx build \
  --platform linux/amd64 \
  -t elasticdotventures/b00t:latest --target $TARGET \
  --build-arg https_proxy=$https_proxy \
  --build-arg http_proxy=$http_proxy \
  -f Dockerfile \
  .
# Example passing secrets:
# https://github.com/moby/buildkit/blob/master/frontend/dockerfile/docs/syntax.md

# docker tag local-image:tagname new-repo:tagname
# docker push new-repo:tagname
docker tag $TARGET _b00t_:latest
docker push _b00t_:latest

exit

## note: could also possibly use docker export to track changes.

cat << EOF
# dev instance, shares common filesystrem. 
docker rm elasticdotventures/b00t
docker run -d -it --name b00t \
 --tmpfs /tmp --tmpfs /run --tmpfs /run/lock  \
 --mount type=bind,source="/c0de",target="/c0de" \
 --privileged \
 -v /var/run/docker.sock:/var/run/docker.sock \
 -v /sys/fs/cgroup:/sys/fs/cgroup:ro \
 elasticdotventures/b00t:latest

 docker exec -it b00t bash --rcfile "./_b00t_.bashrc"

 docker start b00t-run
EOF





# CUSTOM BUILD OUTPUTS?
# * By default, a local container image is created from the build result. 

  # --build-arg "foo"="asdf" \
  #-t b00t -f Dockerfile . 
  # --mount type=bind,source=/c0de/b00t,target=/c0de/b00t \
    
    # --env or --env-file

# docker build -t b00t -f Dockerfile .
# docker run -d -it --name systemd-ubuntu --tmpfs /tmp --tmpfs /run --tmpfs /run/lock  --mount type=bind,source="/c0de",target="/c0de"  --privileged -v /var/run/docker.sock:/var/run/docker.sock -v /sys/fs/cgroup:/sys/fs/cgroup:ro jrei/systemd-ubuntu

# Examples of Post Processing
# docker container create --name extract ???Dunno alexellis2/href-counter:build  
# docker container cp extract:/go/src/github.com/alexellis/href-counter/app ./app  
# docker container rm -f extract
# docker container create --name extract alexellis2/href-counter:build  
# docker build --no-cache -t alexellis2/href-counter:latest .
# rm ./app
