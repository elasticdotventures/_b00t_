---
architecture: dstack is compute orchestration above RunPod/AWS/k8s. Install: uv tool install 'dstack[all]'. Not compatible with HF Jobs — wire as ORCHESTRATOR=dstack separate from ORCHESTRATOR=hf.

---
dstack 0.21.5 kubernetes backend has NO pod-spec injection: compute.py _create_job_pod exposes only resources (cpu/mem/gpu/shm_size), privileged:true, and volumes: mounts that resolve to a dstack-managed PVC (KubernetesVolumeConfiguration) or a hostPath (InstanceMountPoint). No init containers, no inline CSI (csi.spiffe.io), no configMap volume, no serviceAccountName override, no raw pod_spec/patch. Pods that need SPIRE SVID delivery (csi.spiffe.io + spiffe-helper init + ADC ConfigMap + custom SA) MUST be applied via kubectl/Dagster, not dstack apply. Plain identity-free tasks run fine through dstack apply on backends:[kubernetes] once an unconstrained fleet (no resources: block) exists.
