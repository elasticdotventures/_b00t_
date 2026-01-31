mntFree=$(df -k /mnt --output=avail | tail --lines 1
mntUse=$(echo $mntFree / 10 | bc)
sudo dd if=/dev/zero of=/mnt/_LINUX_SWAP_.swap bs=1024 count=$mntUs
sudo swapon /mnt/_LINUX_SWAP_.swap
