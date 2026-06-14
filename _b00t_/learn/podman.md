---
quadlet-podman-kube: preferred node pattern is rootless systemd --user + templated b00t-kube@.kube units + podman kube play manifests under ~/.config/containers/systemd/b00t-kube/

---
nvidia-cdi-drm-drift: if podman GPU containers fail with failed to stat CDI host device /dev/dri/card0 on this node, regenerate /etc/cdi/nvidia.yaml as root; current host exposes /dev/dri/card1 and renderD128.
