output "state_bucket_name" {
  description = "GCS bucket for OpenTofu remote state. Feed this into the root `backend \"gcs\"` block."
  value       = google_storage_bucket.tf_state.name
}

output "state_bucket_url" {
  description = "gs:// URL of the remote-state bucket."
  value       = google_storage_bucket.tf_state.url
}

output "enabled_apis" {
  description = "GCP APIs enabled by this bootstrap."
  value       = sort([for s in google_project_service.api : s.service])
}

output "backend_config" {
  description = "Ready-to-paste root backend block."
  value       = <<-EOT
    terraform {
      backend "gcs" {
        bucket = "${google_storage_bucket.tf_state.name}"
        prefix = "gcp-build-plane"
      }
    }
  EOT
}
