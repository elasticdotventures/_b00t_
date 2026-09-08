variable "project_id" {
  description = "GCP project id that owns the build plane (state bucket + APIs)."
  type        = string
  default     = "promptexecution"
}

variable "region" {
  description = "Canonical region for the whole build plane. The state bucket and every gcp-build-plane resource pin here."
  type        = string
  default     = "australia-southeast1"
}

variable "state_bucket_name" {
  description = "Name of the GCS remote-state bucket. Must match the root module's `backend \"gcs\"` block verbatim (backend blocks cannot interpolate)."
  type        = string
  default     = "b00t-tf-state-promptexecution"
}
