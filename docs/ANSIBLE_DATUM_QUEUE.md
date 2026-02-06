# Ansible Datum Queue

Use `scripts/ansible-datum-status.py` to view the next datum to convert, mark progress, and keep this list current. The script acts like a subagent, reading this table to know what to process next, and it lets you mark entries as Pending / In Progress / Done as you convert them.

## Queue

| Datum | Type | Status | Notes |
| --- | --- | --- | --- |
| ansible.cli | cli | Pending |  |
| argo-cli.cli | cli | Pending |  |
| argo-workflows.k8s | k8s | Pending |  |
| azure-ai-foundry.cli | cli | Pending |  |
| b00t-core-mcps.cli | cli | Pending |  |
| b00t-orchestrator-upgrade.cli | cli | Pending |  |
| fastmcp.cli | cli | Pending |  |
| flux-cd.k8s | k8s | Pending |  |
| git-cliff | cli | Pending |  |
| go.cli | cli | Pending |  |
| just.cli | cli | Pending |  |
| k0s.cli | cli | Pending |  |
| k3d.cli | cli | Pending |  |
| k8s.cli | cli | Done (ansible/playbooks/install-k0s.yaml) | Converted to [ansible/playbooks/install-k0s.yaml]
| k9s.cli | cli | Pending |  |
| kapp.cli | cli | Pending |  |
| kompose.cli | cli | Pending |  |
| kubecost.k8s | k8s | Pending |  |
| kubectx.cli | cli | Pending |  |
| kube-prometheus-stack.k8s | k8s | Pending |  |
| kueue.cli | cli | Pending |  |
| nvidia-gpu-operator.k8s | k8s | Pending |  |
| opentofu.cli | cli | Pending |  |
| python.cli | cli | Done | ansible/playbooks/install-python.yaml |
| rustc.cli | cli | Pending |  |
| stern.cli | cli | Pending |  |
| task.cli | cli | Pending |  |
| terraform.cli | cli | Pending |  |
| uv.cli | cli | Pending |  |
