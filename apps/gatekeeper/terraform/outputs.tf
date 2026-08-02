output "worker_name" {
  value = cloudflare_workers_script.gatekeeper.name
}

output "bucket_name" {
  value = cloudflare_r2_bucket.game_files.name
}

output "kv_namespace_id" {
  value = cloudflare_workers_kv_namespace.limits.id
}
