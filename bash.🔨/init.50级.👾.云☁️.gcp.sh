
## * * * *// 
#* 绿色公司 👾 Google
## * * * *\\

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "./_b00t_.bashrc"


# 🤓 https://cloud.google.com/sdk/docs/downloads-snap
$SUDO_CMD snap install google-cloud-sdk --classic
gcloud init

# TODO: add to .bashrc
# /path/to/gcloud/completion.bash.inc

# Google (the Green Company)
## 绿色公司 //

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


