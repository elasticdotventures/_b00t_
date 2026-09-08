# ---------------------------------------------------------------------------
# gcp-build-plane — dstack control node + build-VM identity + buildcache +
# GitHub WIF + a scale-to-zero pingap "waker" that powers the control node on
# inbound traffic. The VM powers ITSELF off (systemd reaper, Phase 3) when
# dstack has no work — so this module's waker only ever needs to START it.
#
# Plan: docs/... gleaming-jingling-nygaard (rev 2026-09-08).
# Spec: docs/superpowers/specs/2026-08-10-cloud-build-server-design.md
#
# The root module supplies `provider "google"`. No provider block here.
#
# NOT managed here: the dstack `type: volume` persistent disks — `dstack apply`
# owns those; this module only grants the SA `compute.disks.*`.
# ---------------------------------------------------------------------------

locals {
  control_zone = var.control_zone != "" ? var.control_zone : "${var.region}-a"

  # "public" = build phase (external IP, Cloud Run waker). "tailnet" = end state
  # (no external IP, no Cloud Run, firewalls admit only the tailnet). Phase 2.75.
  is_public = var.network_mode == "public"

  # Firewall sources by mode.
  ssh_sources    = local.is_public ? var.allowed_cidrs : [var.tailnet_cidr]
  dstack_sources = local.is_public ? [var.build_plane_subnet_cidr] : [var.tailnet_cidr]

  # dstack's GCP backend permission surface. Sourced from dstack's provider
  # docs. If `dstack apply` later fails with a compute permission error, add the
  # exact permission string here — the coarse fallback is `roles/compute.admin`.
  dstack_backend_permissions = [
    "compute.acceleratorTypes.get",
    "compute.acceleratorTypes.list",
    "compute.addresses.create",
    "compute.addresses.delete",
    "compute.addresses.get",
    "compute.addresses.list",
    "compute.addresses.use",
    "compute.disks.create",
    "compute.disks.delete",
    "compute.disks.get",
    "compute.disks.list",
    "compute.disks.setLabels",
    "compute.disks.use",
    "compute.firewalls.create",
    "compute.firewalls.delete",
    "compute.firewalls.get",
    "compute.firewalls.list",
    "compute.globalOperations.get",
    "compute.images.get",
    "compute.images.list",
    "compute.images.useReadOnly",
    "compute.instances.create",
    "compute.instances.delete",
    "compute.instances.get",
    "compute.instances.list",
    "compute.instances.setLabels",
    "compute.instances.setMetadata",
    "compute.instances.setServiceAccount",
    "compute.instances.setTags",
    "compute.instances.start",
    "compute.instances.stop",
    "compute.machineTypes.get",
    "compute.machineTypes.list",
    "compute.networks.get",
    "compute.networks.list",
    "compute.networks.updatePolicy",
    "compute.projects.get",
    "compute.regionOperations.get",
    "compute.regions.get",
    "compute.regions.list",
    "compute.resourcePolicies.create",
    "compute.resourcePolicies.delete",
    "compute.resourcePolicies.get",
    "compute.resourcePolicies.list",
    "compute.resourcePolicies.use",
    "compute.subnetworks.get",
    "compute.subnetworks.list",
    "compute.subnetworks.use",
    "compute.subnetworks.useExternalIp",
    "compute.zoneOperations.get",
    "compute.zoneOperations.list",
    "compute.zones.get",
    "compute.zones.list",
  ]
}

# ---------------------------------------------------------------------------
# API enablement — the ones gcp-bootstrap (Phase 1) did not turn on. Cloud Run
# + VPC access are needed for the "public"-mode pingap waker. Kept here (not in
# bootstrap) so this module is self-contained; disable_on_destroy = false so a
# `tofu destroy` of the build plane never yanks an API another resource needs.
resource "google_project_service" "build_plane" {
  for_each = toset([
    "run.googleapis.com",
    "vpcaccess.googleapis.com",
  ])
  service                    = each.value
  disable_on_destroy         = false
  disable_dependent_services = false
}

# ---------------------------------------------------------------------------
# Network — a dedicated VPC + one regional subnet. Cloud Run direct VPC egress
# draws its source IPs from this subnet, which is also the tcp/3000 firewall
# source for the waker -> dstack server hop.
# ---------------------------------------------------------------------------
resource "google_compute_network" "build_plane" {
  name                    = "b00t-build-plane"
  auto_create_subnetworks = false
  description             = "b00t GCP build plane — control node + Cloud Run waker + dstack-provisioned build boxes."
}

resource "google_compute_subnetwork" "build_plane" {
  name                     = "b00t-build-plane-${var.region}"
  ip_cidr_range            = var.build_plane_subnet_cidr
  region                   = var.region
  network                  = google_compute_network.build_plane.id
  private_ip_google_access = true
}

# ---------------------------------------------------------------------------
# Service accounts
# ---------------------------------------------------------------------------

# Attached to the control node. Its metadata-server ADC IS the dstack GCP
# backend identity — no key, no WIF for the server.
resource "google_service_account" "dstack_server" {
  account_id   = "b00t-dstack-server"
  display_name = "b00t dstack server (control node)"
  description  = "Metadata-ADC identity for `dstack server` on the control node. Provisions build boxes via the GCP backend."
}

# Attached to every build box dstack provisions (`vm_service_account` in the
# dstack backend config). Gives build boxes metadata ADC for the buildcache.
resource "google_service_account" "build_vm" {
  account_id   = "b00t-build-vm"
  display_name = "b00t build VM"
  description  = "Attached to dstack-provisioned build boxes. Read/write on the buildcache bucket (target/, sccache, ZeroFS state)."
}

# Cloud Run waker. Least privilege: get + start the ONE control instance.
resource "google_service_account" "cp_waker" {
  account_id   = "b00t-cp-waker"
  display_name = "b00t control-plane waker"
  description  = "Cloud Run pingap waker. Starts the control node on inbound dstack traffic. Cannot stop it (the VM self-reaps)."
}

# ---------------------------------------------------------------------------
# IAM — dstack backend custom role
# ---------------------------------------------------------------------------
resource "google_project_iam_custom_role" "dstack_backend" {
  role_id     = "b00tDstackBackend"
  title       = "b00t dstack GCP backend"
  description = "Least-privilege compute surface for `dstack server` to provision/tear down build boxes. Coarse fallback: roles/compute.admin."
  permissions = local.dstack_backend_permissions
}

resource "google_project_iam_member" "dstack_server_backend" {
  project = var.project_id
  role    = google_project_iam_custom_role.dstack_backend.id
  member  = "serviceAccount:${google_service_account.dstack_server.email}"
}

# dstack must attach the build_vm SA to the boxes it creates => actAs.
resource "google_service_account_iam_member" "dstack_server_actas_build_vm" {
  service_account_id = google_service_account.build_vm.name
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${google_service_account.dstack_server.email}"
}

# ---------------------------------------------------------------------------
# IAM — waker custom role, bound only to the control instance
# ---------------------------------------------------------------------------
resource "google_project_iam_custom_role" "cp_waker" {
  role_id     = "b00tCpWaker"
  title       = "b00t control-plane waker"
  description = "get + start a Compute instance. No stop, no delete (the VM self-reaps)."
  permissions = [
    "compute.instances.get",
    "compute.instances.start",
    "compute.zoneOperations.get",
  ]
}

resource "google_project_iam_member" "cp_waker_start" {
  project = var.project_id
  role    = google_project_iam_custom_role.cp_waker.id
  member  = "serviceAccount:${google_service_account.cp_waker.email}"

  condition {
    title       = "only-the-control-instance"
    description = "Restrict get/start to the single control-node VM."
    expression  = "resource.name == \"projects/${var.project_id}/zones/${local.control_zone}/instances/${google_compute_instance.control.name}\""
  }
}

# ---------------------------------------------------------------------------
# Control node
# ---------------------------------------------------------------------------
resource "google_compute_instance" "control" {
  name         = "b00t-dstack-control"
  machine_type = var.control_machine_type
  zone         = local.control_zone
  description  = "b00t dstack server. Powered on by the pingap waker on demand; powers itself off via the systemd reaper when dstack is idle."
  tags         = ["dstack-control"]

  # The waker/reaper stop-start this box out of band; setting a desired_status
  # and ignoring drift keeps `tofu apply` from fighting them.
  desired_status            = "RUNNING"
  allow_stopping_for_update = true

  boot_disk {
    initialize_params {
      image = var.control_boot_image
      size  = var.control_boot_disk_gb
      type  = "pd-standard"
    }
  }

  network_interface {
    subnetwork = google_compute_subnetwork.build_plane.id

    # Ephemeral external IP only in "public" (build-phase) mode — for the
    # pyinfra provisioning SSH and the break-glass tunnel; tcp/22 is firewalled
    # to var.allowed_cidrs. In "tailnet" mode there is no external IP; reach the
    # node over Tailscale (Phase 2.75).
    dynamic "access_config" {
      for_each = local.is_public ? [1] : []
      content {}
    }
  }

  service_account {
    email  = google_service_account.dstack_server.email
    scopes = ["cloud-platform"]
  }

  metadata = {
    ssh-keys               = var.control_authorized_ssh_keys
    block-project-ssh-keys = "true"
    enable-oslogin         = "false"
  }

  labels = var.labels

  lifecycle {
    # The reaper flips this to TERMINATED; do not let apply restart it.
    ignore_changes = [desired_status]
  }
}

# ---------------------------------------------------------------------------
# Firewalls
# ---------------------------------------------------------------------------
resource "google_compute_firewall" "control_ssh" {
  count   = length(local.ssh_sources) > 0 ? 1 : 0
  name    = "b00t-control-ssh"
  network = google_compute_network.build_plane.name

  allow {
    protocol = "tcp"
    ports    = ["22"]
  }

  source_ranges = local.ssh_sources
  target_tags   = ["dstack-control"]
  description   = local.is_public ? "Operator SSH to the control node (build phase)." : "Tailnet SSH to the control node."
}

resource "google_compute_firewall" "control_dstack" {
  name    = "b00t-control-dstack"
  network = google_compute_network.build_plane.name

  allow {
    protocol = "tcp"
    ports    = ["3000"]
  }

  # public: Cloud Run direct VPC egress -> source IP from the build-plane subnet.
  # tailnet: only the Tailscale CGNAT range. Never the public internet.
  source_ranges = local.dstack_sources
  target_tags   = ["dstack-control"]
  description   = "dstack server ingress — pingap waker (public) or tailnet only. NOT open to the internet."
}

# ---------------------------------------------------------------------------
# Buildcache bucket
# ---------------------------------------------------------------------------
resource "google_storage_bucket" "buildcache" {
  name                        = var.buildcache_bucket_name
  location                    = var.region
  storage_class               = "STANDARD"
  force_destroy               = false
  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"

  dynamic "lifecycle_rule" {
    for_each = var.buildcache_age_delete_days > 0 ? [1] : []
    content {
      condition {
        age = var.buildcache_age_delete_days
      }
      action {
        type = "Delete"
      }
    }
  }

  labels = merge(var.labels, { purpose = "buildcache" })
}

resource "google_storage_bucket_iam_member" "build_vm_objadmin" {
  bucket = google_storage_bucket.buildcache.name
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${google_service_account.build_vm.email}"
}

resource "google_storage_bucket_iam_member" "ci_objviewer" {
  bucket = google_storage_bucket.buildcache.name
  role   = "roles/storage.objectViewer"
  member = "serviceAccount:${google_service_account.ci.email}"
}

# ---------------------------------------------------------------------------
# GitHub Actions -> GCP, keyless (Workload Identity Federation)
# ---------------------------------------------------------------------------
resource "google_iam_workload_identity_pool" "github" {
  workload_identity_pool_id = "github-actions"
  display_name              = "GitHub Actions"
  description               = "Keyless OIDC federation for GitHub Actions in ${var.github_repository}."
}

resource "google_iam_workload_identity_pool_provider" "github_oidc" {
  workload_identity_pool_id          = google_iam_workload_identity_pool.github.workload_identity_pool_id
  workload_identity_pool_provider_id = "github-oidc"
  display_name                       = "GitHub OIDC"

  attribute_mapping = {
    "google.subject"       = "assertion.sub"
    "attribute.repository" = "assertion.repository"
    "attribute.ref"        = "assertion.ref"
  }

  # Required by the provider whenever attribute.* mappings are present.
  attribute_condition = "assertion.repository == \"${var.github_repository}\""

  oidc {
    issuer_uri = "https://token.actions.githubusercontent.com"
  }
}

resource "google_service_account" "ci" {
  account_id   = "b00t-ci"
  display_name = "b00t CI (GitHub Actions, WIF)"
  description  = "Impersonated keylessly by GitHub Actions in ${var.github_repository}. Read-only on the buildcache."
}

resource "google_service_account_iam_member" "ci_wif" {
  service_account_id = google_service_account.ci.name
  role               = "roles/iam.workloadIdentityUser"
  member             = "principalSet://iam.googleapis.com/${google_iam_workload_identity_pool.github.name}/attribute.repository/${var.github_repository}"
}

# ---------------------------------------------------------------------------
# Scale-to-zero pingap waker (Cloud Run v2) — "public" mode only. In "tailnet"
# mode the waker is a tailnet service on b00t-node minted from the cp_waker SA
# (Phase 2.75); the SA + role below stay so that key can be issued.
# ---------------------------------------------------------------------------
resource "google_cloud_run_v2_service" "cp_waker" {
  count      = local.is_public ? 1 : 0
  name       = "b00t-cp-waker"
  location   = var.region
  ingress    = "INGRESS_TRAFFIC_ALL"
  depends_on = [google_project_service.build_plane]

  template {
    service_account = google_service_account.cp_waker.email

    scaling {
      min_instance_count = 0
      max_instance_count = 1
    }

    vpc_access {
      network_interfaces {
        network    = google_compute_network.build_plane.id
        subnetwork = google_compute_subnetwork.build_plane.id
      }
      egress = "PRIVATE_RANGES_ONLY"
    }

    containers {
      image = var.waker_image

      env {
        name  = "PROJECT_ID"
        value = var.project_id
      }
      env {
        name  = "CONTROL_ZONE"
        value = local.control_zone
      }
      env {
        name  = "CONTROL_INSTANCE"
        value = google_compute_instance.control.name
      }
      # pingap upstream — the control node's INTERNAL address, reached over
      # direct VPC egress.
      env {
        name  = "DSTACK_UPSTREAM_ADDR"
        value = "${google_compute_instance.control.network_interface[0].network_ip}:3000"
      }
      env {
        name  = "IDLE_GRACE"
        value = var.idle_grace
      }
      env {
        name  = "READINESS_PATH"
        value = "/healthz"
      }
      env {
        name  = "START_DEADLINE_SECONDS"
        value = "120"
      }
    }
  }

  lifecycle {
    # First apply may race the image not existing yet; the operator builds and
    # pushes containers/b00t-cp-waker/ then re-applies. Keep manual revisions.
    ignore_changes = [client, client_version]
  }
}

resource "google_cloud_run_v2_service_iam_member" "waker_public" {
  count    = local.is_public && var.waker_public ? 1 : 0
  name     = google_cloud_run_v2_service.cp_waker[0].name
  location = google_cloud_run_v2_service.cp_waker[0].location
  role     = "roles/run.invoker"
  member   = "allUsers"
}
