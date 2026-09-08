output "control_node_name" {
  description = "Control-node instance name."
  value       = google_compute_instance.control.name
}

output "control_zone" {
  description = "Control-node zone."
  value       = local.control_zone
}

output "control_node_external_ip" {
  description = "Ephemeral external IP of the control node in network_mode=\"public\" (pyinfra SSH, break-glass tunnel). Empty in \"tailnet\" mode — reach it by MagicDNS name."
  value       = try(google_compute_instance.control.network_interface[0].access_config[0].nat_ip, "")
}

output "control_node_internal_ip" {
  description = "Internal IP of the control node (pingap waker upstream)."
  value       = google_compute_instance.control.network_interface[0].network_ip
}

output "control_node_endpoint" {
  description = "Public pingap waker URL in network_mode=\"public\" — `dstack project add --url` target; the waker powers the VM on first call. Empty in \"tailnet\" mode (reach dstack at http://<magicdns>:3000)."
  value       = local.is_public ? one(google_cloud_run_v2_service.cp_waker[*].uri) : ""
}

output "buildcache_bucket" {
  description = "Shared build-cache GCS bucket."
  value       = google_storage_bucket.buildcache.name
}

output "dstack_server_sa_email" {
  description = "SA attached to the control node (dstack GCP backend identity)."
  value       = google_service_account.dstack_server.email
}

output "build_vm_sa_email" {
  description = "SA to set as `vm_service_account` in the dstack GCP backend config."
  value       = google_service_account.build_vm.email
}

output "ci_sa_email" {
  description = "SA that GitHub Actions impersonates via WIF."
  value       = google_service_account.ci.email
}

output "wif_provider_name" {
  description = "Full resource name of the WIF provider — the `workload_identity_provider` input for google-github-actions/auth."
  value       = google_iam_workload_identity_pool_provider.github_oidc.name
}

output "wif_pool_name" {
  description = "Full resource name of the WIF pool."
  value       = google_iam_workload_identity_pool.github.name
}

output "network_self_link" {
  description = "Build-plane VPC self-link (for the dstack backend `vpc_name`)."
  value       = google_compute_network.build_plane.self_link
}

output "subnetwork_self_link" {
  description = "Build-plane subnet self-link."
  value       = google_compute_subnetwork.build_plane.self_link
}
