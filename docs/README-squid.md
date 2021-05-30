README-squid.

# https://hub.docker.com/r/sameersbn/squid
SQUID is an http/https caching proxy.
it is used in build.sh
to cache repos during build.

docker pull sameersbn/squid:3.5.27-2

docker run --name squid -d --restart=always \
  --publish 3128:3128 \
  --volume /srv/docker/squid/cache:/var/spool/squid \
  sameersbn/squid:3.5.27-2

export ftp_proxy=http://172.17.0.1:3128
export http_proxy=http://172.17.0.1:3128
export https_proxy=http://172.17.0.1:3128

