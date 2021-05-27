
## * * * *// 
#* 绿色公司 👾 Google
## * * * *\\

## Don't install k8 locally and bloat up the image /snap/bin/gcloud can't install sub-components such as "kubectl"
# 🤓 https://cloud.google.com/sdk/docs/downloads-snap
# $SUDO_CMD snap install google-cloud-sdk --classic
# gcloud init
# gcloud components install kubectl

## the DOCKER approach for gcloud is also shit, because (as configured) it can't remember credentials between runs
# 🤓 https://hub.docker.com/r/google/cloud-sdk/
# docker pull google/cloud-sdk:latest
# docker run -ti --name gcloud-config google/cloud-sdk gcloud auth login
# docker run --rm -ti --volumes-from gcloud-config google/cloud-sdk gcloud compute instances list --project your_project

## finally gcloud has bashrc complete, which is a really cool feature, that requires it be installed like this: 
# TODO: add to .bashrc
# /path/to/gcloud/completion.bash.inc
## add cloud sdk distribution URL as a package source
echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main" | sudo tee -a /etc/apt/sources.list.d/google-cloud-sdk.list
## import google cloud public key 
curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo apt-key --keyring /usr/share/keyrings/cloud.google.gpg add -
## these other packages are ALSO available: 
#    google-cloud-sdk-app-engine-python
#    google-cloud-sdk-app-engine-python-extras
#    google-cloud-sdk-app-engine-java
#    google-cloud-sdk-app-engine-go
#    google-cloud-sdk-bigtable-emulator
#    google-cloud-sdk-cbt
#    google-cloud-sdk-cloud-build-local
#    google-cloud-sdk-datalab
#    google-cloud-sdk-datastore-emulator
#    google-cloud-sdk-firestore-emulator
#    google-cloud-sdk-pubsub-emulator
#    kubectl
$SUDO_CMD apt-get install google-cloud-sdk kubectl 

# Are ProtoBufs strictly a Google thing? 
# https://developers.google.com/protocol-buffers/
# https://github.com/protocolbuffers/protobuf
# https://blog.reverberate.org/2021/04/21/musttail-efficient-interpreters.html
# https://github.com/protocolbuffers/protobuf/releases/tag/v3.15.8
# Parsing Protobuf at 2+GB/s: 
# https://blog.reverberate.org/2021/04/21/musttail-efficient-interpreters.html

PROTOBUF_VERSION="3.15.8"
cd /usr/local/src
wget https://github.com/protocolbuffers/protobuf/releases/download/v3.15.8/protobuf-all-$PROTOBUF_VERSION.tar.gz
tar -xzf protobuf-all-$PROTOBUF_VERSION.tar.gz
cd protobuf-$PROTOBUF_VERSION/
./configure
make install
cd ..

# Google Drive FS Mount
# note: it's embarassing Google doesn't offer their own
# https://www.techrepublic.com/article/how-to-mount-your-google-drive-on-linux-with-google-drive-ocamlfuse/#:~:text=Mounting%20your%20Google%20Drive&text=From%20the%20terminal%2C%20issue%20the,to%20the%20google%2Ddrive%20folder.

## ocamlfuse Google Drive Mount
#sudo add-apt-repository ppa:alessandro-strada/ppa
#sudo apt-get install google-drive-ocamlfuse
#sudo mkdir ~/google.👾.drive
#google-drive-ocamlfuse ~/google.👾.drive
# fusermount -u ~/migoogledrive

## GCSF https://github.com/harababurel/gcsf
sudo apt install -y libfuse-dev pkg-config

# Future: 
# https://github.com/google/or-tools

