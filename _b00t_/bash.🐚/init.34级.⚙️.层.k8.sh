# should be run by _b00t_
source "$_B00T_C0DE_Path/_b00t_.bashrc"

## * * * * * * 
## kubectl 
echo "deb [signed-by=/usr/share/keyrings/kubernetes-archive-keyring.gpg] https://apt.kubernetes.io/ kubernetes-xenial main" | sudo tee /etc/apt/sources.list.d/kubernetes.list
$SUDO_CMD curl -fsSLo /usr/share/keyrings/kubernetes-archive-keyring.gpg https://packages.cloud.google.com/apt/doc/apt-key.gpg
$SUDO_CMD apt-get update
$SUDO_CMD apt-get install -y kubectl

## This is the *same* non-APT route: 
# curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl
## verify binary 
# curl -LO "https://dl.k8s.io/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl.sha256"
# echo "$(<kubectl.sha256) kubectl" | sha256sum --check
## install
# $SUDO_CMD install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# OR install local
#$ mkdir -p ~/.local/bin/kubectl
#$ mv ./kubectl ~/.local/bin/kubectl
#$ pathAdd ~/.local/bin/kubectl to $PATH

 kubectl get namespaces -o json | jq '.items[] | [ .metadata.name, .status.phase ] | @tsv'



## * * * * * * 



# TODO: check version
# kubectl version --client
# Client Version: version.Info{Major:"1", Minor:"21", GitVersion:"v1.21.1", GitCommit:"5e58841cce77d4bc13713ad2b91fa0d961e69192", GitTreeState:"clean", BuildDate:"2021-05-12T14:18:45Z", GoVersion:"go1.16.4", Compiler:"gc", Platform:"linux/amd64"}

## * * * * * * 
## skaffold

# docker run gcr.io/k8s-skaffold/skaffold:latest skaffold <command>
sudo apt-get install google-cloud-sdk-skaffold

## * * * * * * 
# Minikube
# note: there are ARM64, etc. versions, however I can't imagine any situation where that's useful presently. 
curl -LO https://storage.googleapis.com/minikube/releases/latest/minikube_latest_amd64.deb
sudo dpkg -i minikube_latest_amd64.deb

# 'start cluster'
# minikube start
# kubectl get po -A

# Virtual Kublet, an open source kubernetes kubelet, multi-cloud
#https://virtual-kubelet.io/

# this is mostly a placeholder since k8 b0rgification is inenvitable.

# _b00t_ will interface with k8 via argo workflows "submodules"
# https://github.com/argoproj/argo-workflows

## * * * * * * 
# HELM 
# https://helm.sh/docs/intro/install/
curl https://baltocdn.com/helm/signing.asc | sudo apt-key add -
sudo apt-get install apt-transport-https --yes
echo "deb https://baltocdn.com/helm/stable/debian/ all main" | sudo tee /etc/apt/sources.list.d/helm-stable-debian.list
sudo apt-get update
sudo apt-get install helm


# Argo  "Arrgho!"

replicas: 1
strategy: 
    type: RollingUpdate



# Argo/Helm Charts: 
# https://github.com/argoproj/argo-helm

# Couler?
# https://github.com/couler-proj/couler

# Apache Airflow (works with Couler)
# https://airflow.apache.org/docs/apache-airflow/stable/start/docker.html
# so airflow requires postgres

# KubeFlow (works with airflow, couler)
# https://www.kubeflow.org/docs/components/pipelines/installation/overview/#google-cloud-ai-platform-pipelines

## Not sure DroneCI will make cut, 
# DRONE_GITHUB_CLIENT_ID
# DRONE_GITHUB_CLIENT_SECERT
# RPC_SECRET
# USER_CREATE
# 
echo "username: marcel-dempers,admin:true" | base64 -w 0
Drone_Server