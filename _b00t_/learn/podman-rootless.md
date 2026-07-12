---
userns: --userns=keep-id not --user for bind-mount writes in rootless podman

---
chmod-777: chmod 777 output dirs before training; rootless podman remaps uid so --user 1000 inside != host uid 1000; 777 allows any uid to write; --userns=keep-id silently blocks CDI GPU access
