https://github.com/remote-android/redroid-doc#troubleshooting

# Ubuntu 20.04+ (kernel 5.0+)
sudo modprobe ashmem_linux
sudo modprobe binder_linux devices=binder,hwbinder,vndbinder
