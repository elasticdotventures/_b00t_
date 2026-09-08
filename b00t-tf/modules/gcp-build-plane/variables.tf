# gcp-build-plane — inputs. Defaults encode the operator decisions in
# plan gleaming-jingling-nygaard (2026-09-08).

variable "project_id" {
  description = "GCP project that owns the build plane."
  type        = string
  default     = "promptexecution"
}

variable "region" {
  description = "Canonical region. Every resource here pins to it (a network volume can only attach to compute in its own region)."
  type        = string
  default     = "australia-southeast1"
}

variable "control_zone" {
  description = "Zone for the control-node VM. Empty => \"<region>-a\"."
  type        = string
  default     = ""
}

variable "control_machine_type" {
  description = "Control-node machine type. e2-small (2 GB) runs `dstack server` + SQLite for a single build box; bump to e2-medium (4 GB) if a real provision cycle won't fit."
  type        = string
  default     = "e2-small"
}

variable "control_boot_image" {
  description = "Control-node boot image (family or full self-link)."
  type        = string
  default     = "debian-cloud/debian-13"
}

variable "control_boot_disk_gb" {
  description = "Control-node boot disk size (GiB). dstack server + deps + SQLite is < 2 GB; the disk is billed while the VM is powered off."
  type        = number
  default     = 10
}

variable "control_authorized_ssh_keys" {
  description = "Value for the control-node `ssh-keys` metadata: newline-joined \"<user>:<ssh-...>\" lines. Rendered by the root module from _b00t_/keyring.tomllm. Empty = no per-user keys (access via a later pyinfra sync or project keys only)."
  type        = string
  default     = ""
}

variable "allowed_cidrs" {
  description = "CIDRs allowed to reach the control node on tcp/22 while network_mode=\"public\". Keep tight (operator egress). Empty = no SSH ingress rule. Ignored when network_mode=\"tailnet\"."
  type        = list(string)
  default     = []
}

variable "network_mode" {
  description = "\"public\" (build phase): control node gets an ephemeral external IP, waker is a public Cloud Run service, firewalls admit allowed_cidrs / the subnet. \"tailnet\" (end state): no external IP, no Cloud Run waker, firewalls admit only tailnet_cidr — reach everything over Tailscale (Phase 2.75)."
  type        = string
  default     = "public"

  validation {
    condition     = contains(["public", "tailnet"], var.network_mode)
    error_message = "network_mode must be \"public\" or \"tailnet\"."
  }
}

variable "tailnet_cidr" {
  description = "Tailscale CGNAT range. The only firewall source when network_mode=\"tailnet\"."
  type        = string
  default     = "100.64.0.0/10"
}

variable "build_plane_subnet_cidr" {
  description = "Primary range for the build-plane subnet. Cloud Run direct-VPC-egress source IPs come from here, so it is also the tcp/3000 firewall source for the pingap waker -> dstack server."
  type        = string
  default     = "10.20.0.0/24"
}

variable "buildcache_bucket_name" {
  description = "GCS bucket for the shared build cache. ZeroFS uses prefix `zerofs/`; CI/manual sccache uses `sccache/`."
  type        = string
  default     = "b00t-buildcache-promptexecution"
}

variable "buildcache_age_delete_days" {
  description = "Age (days) after which buildcache objects are deleted. 0 disables the lifecycle rule."
  type        = number
  default     = 30
}

variable "github_repository" {
  description = "owner/repo that the WIF provider will trust for keyless GitHub Actions -> GCP auth."
  type        = string
  default     = "elasticdotventures/_b00t_"
}

variable "waker_image" {
  description = "Container image for the pingap scale-to-zero waker (Cloud Run). Build from containers/b00t-cp-waker/."
  type        = string
  default     = "ghcr.io/promptexecution/b00t-cp-waker:latest"
}

variable "waker_public" {
  description = "When true the waker Cloud Run URL is invokable by allUsers (it is the public `dstack` endpoint; dstack's own token still gates the API). When false, grant run.invoker out of band."
  type        = bool
  default     = true
}

variable "idle_grace" {
  description = "Passed to the VM self-reaper as IDLE_GRACE. Must exceed the dstack fleet `idle_duration` (30m) so the control node outlives the build box it is tearing down."
  type        = string
  default     = "45m"
}

variable "labels" {
  description = "Labels applied to labelable resources."
  type        = map(string)
  default = {
    managed-by = "opentofu"
    module     = "gcp-build-plane"
  }
}
