#!/usr/bin/bash


## * * * *// 
#* 绿色公司 👾 Google
## * * * *\\

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "$_B00T_C0DE_Path/_b00t_.bashrc"

# local cloud, kubernetes, etc.
## syncthing

sudo apt install curl apt-transport-https

# wonky: ?? sudo didn't work? 
# echo "deb https://apt.syncthing.net/ syncthing release" | sudo cat > /etc/apt/sources.list.d/syncthing.list
syncthing --version

# https://hub.docker.com/r/linuxserver/syncthing
docker run -d \
  --name=syncthing \
  --hostname=Sm3llS1k3S01d3r \
  -e PUID=1000 \
  -e PGID=1000 \
  -e TZ=AEST \
  -p 8384:8384 \
  -p 22000:22000/tcp \
  -p 22000:22000/udp \
  -p 21027:21027/udp \
  -v /mnt/syncthing/config:/config \
  -v /mnt/c0re:/mnt/c0re \
  --restart unless-stopped \
  ghcr.io/linuxserver/syncthing


  --sysctl="net.core.rmem_max=2097152" \

 
:/mmediting# 
python demo/inpainting_demo.py ${CONFIG_FILE^C${CHECKPOINT_FILE} ${MASKED_IMAGE_FILE} ${MASK_FILE} ${SAVE_FILE} [--imshow] [--device ${GPU_ID}]
