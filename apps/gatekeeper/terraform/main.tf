terraform {
  required_version = ">= 1.5"
  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
  }
}

provider "cloudflare" {
  # API token comes from the CLOUDFLARE_API_TOKEN environment variable.
  # It is never stored in this repository or in any .tf file.
  # account_id is passed per resource below.
}

resource "cloudflare_r2_bucket" "game_files" {
  account_id = var.account_id
  name       = var.bucket_name
}

resource "cloudflare_workers_kv_namespace" "limits" {
  account_id = var.account_id
  title      = var.kv_namespace_title
}

resource "cloudflare_workers_script" "gatekeeper" {
  account_id = var.account_id
  name       = var.worker_name
  # Built bundle. Run `bun run build` in apps/gatekeeper before `terraform apply`.
  content = file("../dist/index.js")
  module  = true

  r2_bucket_binding {
    name        = "GAME_FILES"
    bucket_name = cloudflare_r2_bucket.game_files.name
  }

  kv_namespace_binding {
    name         = "LIMITS"
    namespace_id = cloudflare_workers_kv_namespace.limits.id
  }

  dynamic "plain_text_binding" {
    for_each = var.allowed_origins == "" ? [] : [1]
    content {
      name = "ALLOWED_ORIGINS"
      text = var.allowed_origins
    }
  }
}
