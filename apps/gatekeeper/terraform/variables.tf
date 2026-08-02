variable "account_id" {
  description = "Cloudflare account ID that owns the worker, bucket, and KV namespace."
  type        = string
}

variable "worker_name" {
  description = "Name of the Workers script. Must match the name in wrangler.toml for wrangler secret put."
  type        = string
  default     = "doraemon-gatekeeper"
}

variable "bucket_name" {
  description = "R2 bucket that holds the original game files."
  type        = string
  default     = "doraemon-game-files"
}

variable "kv_namespace_title" {
  description = "Title of the KV namespace used for rate limiting."
  type        = string
  default     = "doraemon-gatekeeper-limits"
}

variable "allowed_origins" {
  description = "Comma-separated list of origins allowed to read files (empty allows any Origin header, CLI calls included)."
  type        = string
  default     = ""
}
