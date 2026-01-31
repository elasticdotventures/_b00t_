
# 鲸鱼 - Jīngyú - Docker!  🐳

The Whale is Dockers official mascot & spirit animal. 
As such the 鲸鱼 (Whale) storytime log/namespace

## get started with b00t

Docker creates "containers" for operating systems and installed packages. The containers start with a _base image_ and then are built by running a series of `RUN` commands to create _Layers_. 

Containers are stored in servers known as Docker Repositories, similar to how Git projects are stored in hosted git repositories.  Docker repositories provide the container images to docker build processes. Docker by default uses docker.io also called "Docker Hub" as a registry which allows publishing of public Docker repos for free, and paid private plans.  If you publish a container which contains any KEYs, PASSWORDs, secret, etc. then hacker-bots can extract those from your container - so don't do that! _b00t_ v1 will attempt to establish a private registry using Azure Container Registry (ACR).

Docker Layers behave similary to Git Branches or Python Virtual Environments. Anything that happens in a Docker container stays inside thatt container.  Docker containers by default expose no services and you must `EXPOSE` ports for ssh, http, https, etc. 

Docker is smart - when updating/rebuilding a docker container only the layer which has changed and ALL layers following the modified layer are rebuilt.  Docker will often be a part of a developers "inner-loop" (rebuilding an image after each change).  To keep fast build times it's best to have layers which will change a lot towards the bottom/end. Usually this is why you're own layer (that you're developing) is last.  

One important reason to use Docker is that the resulting containers are isolated from other containers.  _b00t_ believes that Idempotent containers - those which perform one task (or a series of similar tasks, using idempotent functions) and then shut-down are BEST and it is undesirable to have long running containers which handle many types of requests from a variety of users. 

Containers can be booted, then stopped or "frozen" at the point they are ready & waiting for a request.  Then copies of the frozen container can be re-loaded faster than actually booting (or even have Hot-standbys that are pre-thawed to handle subsequent requests), or a docker server can be left running to handle many requests (such as gunicorn, uwsgi) where the request isolation happens at the 

Docker-compose.yml, Azure Container Instances (ACI), and AWS ECS (Elastic Container Service) are both services for starting one or more containers and are built on top of docker-compose, which is part of "Swarm".  Docker Swarm itself is End-of-Life already but it's lineage lives on through ACI, ECS and others, although each service has it's own unique customizations.

Kubernetes, groups containers together into "pods". The term "pods" is distinct to K8.  "K8" is Googles open-source contribution to container management - built on Docker Swarm. K8 is consider the direct successor by most people to Docker Swarm. k8 is also offer as a managed service by Azure Kubernetes Services (AKS) which is distinct from ACI.

_b00t_ approach is that k8 exists MOSTLY to accomodate older software architectures which require a plurality of instances running simulatenously to complete a task (distinctly "non-idempotent").  SO if you're building new software app using _b00t_ it's almost certainly better to use a serverless HTTP function so you don't even need to THINK about the operating system.  K8 is not presently part of _b00t_ despite it being extremely popular among prospective employers it (in the opinion of the _b00t_ author) only really suitable to operate in companies that are equivalently sized to Google. If your company is smaller than Google it's probably better to use a managed K8 service from your preferred cloud provider UNLESS you life plan is to become a professional administrator of docker pods.  There is almost always a better pattern and way to do things than starting a new build on K8. 

# Future: 
# https://github.com/nestybox/sysbox
# https://github.com/kata-containers/kata-containers
# gvisor - probably not.


## +++ MultiStage Builds
# also notes on chaining files, etc.
https://docs.docker.com/develop/develop-images/multistage-build/

# Really useful docker command list:
https://spin.atomicobject.com/2018/10/04/docker-command-line/