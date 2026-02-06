

### Azure Container Registry:   
  elasticdotventures.azurecr.io

## Running Docker in Docker
* https://devopscube.com/run-docker-in-docker/#:~:text=To%20run%20docker%20inside%20docker,sock%20as%20a%20volume.&text=Just%20a%20word%20of%20caution,privileges%20over%20your%20docker%20daemon.
* Nesty Sysbox:
https://github.com/nestybox/sysbox

# Documentation & Tutorials:

## Whats the difference between Runc, Containerd, & Docker
* https://alenkacz.medium.com/whats-the-difference-between-runc-containerd-docker-3fc8f79d4d6e

## Kata Containers on MS Azure
* https://github.com/kata-containers/kata-containers/blob/main/docs/install/azure-installation-guide.md
* Ubuntu: https://github.com/kata-containers/documentation/blob/master/install/docker/ubuntu-docker-install.md

## Installing and Using MariaDB via Docker
* https://mariadb.com/kb/en/installing-and-using-mariadb-via-docker/

## Mount a secret volume in Azure Container Instances
  * Requires Container Group 
  https://docs.microsoft.com/en-us/azure/container-instances/container-instances-volume-secret

# Container groups in Azure Container Instances
https://docs.microsoft.com/en-us/azure/container-instances/container-instances-container-groups
* YAML
https://docs.microsoft.com/en-us/azure/container-instances/container-instances-reference-yaml


Really useful docker command list:
https://spin.atomicobject.com/2018/10/04/docker-command-line/

docker run -it --rm -p 8080:8080 sportsworldapp
docker run -it --rm -p 8000:80 sportsworldapp:latest
docker build . -f Dockerfile -t sportsworldapp:latest


# https://github.com/nestybox/sysbox
# https://github.com/kata-containers/kata-containers
# gvisor - probably not.


## +++ MultiStage Builds
# also notes on chaining files, etc.
https://docs.docker.com/develop/develop-images/multistage-build/
