---
k0s image import without sudo: run a pod that hostPath-mounts /run/k0s + /usr/local/bin/k0s + the podman-save tarball, then '/host/k0s ctr -n k8s.io images import /import/x.tar'; also podman save keeps localhost/ prefix so manifests need image: localhost/<name>
