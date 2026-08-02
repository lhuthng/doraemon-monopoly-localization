# Doraemon Monopoly gatekeeper worker

A Cloudflare Worker + R2 gatekeeper that serves the private original game files
(`workspace/base/`) to authorized callers. It is an **optional second way** to
obtain game files — the manual flow (drop your own files into `workspace/base/`)
remains the primary one.

Three access modes:

| Caller | Credential | Flow |
| --- | --- | --- |
| Resource Studio (CLI) | `Authorization: Bearer <secret>` | `make fetch-base` populates `workspace/base/` |
| Translator Workshop (browser) | `X-Coupon: <coupon>` | "Project coupon" UI fetches `strings.dat`, `voice.dat`, `sysfont.dat` |
| Anyone | none | `GET /api/health` only |

## Layout

| Path | Role |
| --- | --- |
| `src/index.ts` | Worker: auth, allowlist, R2 streaming, rate limiting, CORS |
| `scripts/mint-coupon.ts` | Print a random coupon + SHA-256 without activating it |
| `scripts/add-coupon.ts` | Mint (or take) a coupon, record it, push it live |
| `scripts/delete-coupon.ts` | Revoke a coupon and push immediately |
| `scripts/list-coupons.ts` | Show active vs revoked coupons |
| `scripts/sync-coupon-hashes.ts` | Force-push the active set from the registry |
| `coupons.registry.json` | Local source of truth: coupon → SHA-256 (gitignored) |
| `test/gatekeeper.test.ts` | Unit tests for auth, allowlist, rate limit, CORS |
| `wrangler.toml` | Local `wrangler dev` config (mirrors Terraform bindings) |
| `terraform/` | Terraform: R2 bucket, KV namespace, worker script + bindings |

## Security model

- **No secrets in the repo.** `MAINTAINER_SECRET` and `COUPON_HASHES` are
  Cloudflare Workers Secrets (`wrangler secret put`), never in Terraform state
  or git. The only local copy of coupon plaintext is the gitignored
  `coupons.registry.json` (and gitignored `.env`/`.dev.vars`).
- Coupons are stored **only as SHA-256 digests**. The worker hashes the presented
  coupon and compares constant-time against any hash in the `COUPON_HASHES` JSON
  array, so many coupons can be valid at once.
- Strict 7-file allowlist (`Doraemon.exe`, `strings.dat`, `sysfont.dat`,
  `Sprite1.dat`, `sprite2.dat`, `bitmaps.dat`, `voice.dat`) blocks path traversal.
- Per-IP rate limiting (20 req/min) via a KV counter; returns `429 + Retry-After`.
- CORS allows only configured origins; CLI calls (no `Origin` header) always pass.
- Each object carries `X-SHA256` metadata; `content/base-fingerprints.json` holds
  the public SHA-256 hashes used to verify every download.

## One-time setup

Prereqs: `bun`, `wrangler` (or `bunx wrangler`), `terraform`, a Cloudflare account
with an API token granting Workers Scripts, R2, and Workers KV edit access.

```sh
# 1. Infra (bucket, KV, worker + bindings) via Terraform
cd apps/gatekeeper/terraform
cp terraform.tfvars.example terraform.tfvars   # fill account_id
export CLOUDFLARE_API_TOKEN=...                # never written to files
cd .. && bun run build                         # produces dist/index.js (required by terraform)
cd terraform
terraform init
terraform apply

# 2. Secrets via wrangler (never through Terraform)
cd apps/gatekeeper
wrangler secret put MAINTAINER_SECRET --name doraemon-gatekeeper
# COUPON_HASHES is managed by the coupon commands below (add/list/delete/sync);
# no manual wrangler secret put needed for it.

# 3. Local dev mirror
cp wrangler.toml.example wrangler.toml          # fill KV namespace id from terraform output
cp .dev.vars.example .dev.vars                  # MAINTAINER_SECRET for `wrangler dev`
# (COUPON_HASHES in .dev.vars is auto-maintained by the coupon commands)
```

> `wrangler.toml` mirrors Terraform bindings for `wrangler dev` only. Terraform is
> the source of truth for the deployed worker; keep the two in sync manually.

## Minting coupons

Credentials first — every coupon command pushes to Cloudflare, so it needs an
API token. Put it once in `apps/gatekeeper/.env` (copy `.env.example`, never
commit — Bun auto-loads it):

```sh
# apps/gatekeeper/.env
CLOUDFLARE_API_TOKEN=...            # API token with Workers Scripts edit rights
CLOUDFLARE_ACCOUNT_ID=...           # optional: falls back to terraform.tfvars / .dev.vars
```

Then:

```sh
make gatekeeper-add-coupon                 # random coupon, minted + live now
make gatekeeper-add-coupon COUPON="Phrase" # human-readable coupon, minted + live now
```

`apps/gatekeeper/coupons.registry.json` (gitignored — it holds plaintext
coupons) is the single source of truth. Every mint/revoke derives the active
`COUPON_HASHES` array from it, rewrites `.dev.vars` as a mirror, and pushes the
worker secret over the Cloudflare API — so it never drifts from what's live. If
the token is missing, the command fails with a clear message instead of
printing a manual step.

### Listing and revoking coupons

```sh
make gatekeeper-list-coupons                   # who is active, who is revoked
make gatekeeper-delete-coupon COUPON="Phrase"  # revoke by plaintext coupon
make gatekeeper-delete-coupon HASH=<sha256>    # revoke by full hash (e.g. legacy coupons)
make gatekeeper-sync-coupons                   # force-push the current active set
```

Revoking marks the entry revoked and re-pushes; the digest is removed from the
worker secret so the coupon stops working immediately. Cloudflare only ever
holds digests. `make gatekeeper-mint` prints a raw coupon + hash without
activating anything, for a quick one-off.

## Uploading the game files into R2

```sh
# apps/resource-studio/.env  (never committed)
R2_ACCOUNT_ID=...
R2_ACCESS_KEY_ID=...
R2_SECRET_ACCESS_KEY=...
R2_BUCKET=doraemon-game-files

make upload-base
```

Uploads the 7 files from `workspace/base/`, verifies each against
`content/base-fingerprints.json`, and stores the SHA-256 as object metadata so
the worker can serve it back as `X-SHA256`.

## Consuming the files

```sh
# Studio: populate workspace/base/ then prepare as usual
make fetch-base          # reads CLOUDFLARE_GATEKEEPER_URL / CLOUDFLARE_GATEKEEPER_SECRET
make prepare
```

Workshop: build with `PUBLIC_GATEKEEPER_URL=https://<worker>.<account>.workers.dev`
(`apps/translator-workshop/.env`, never committed). The "Project coupon" section
then fetches and loads the three files through the existing in-browser pipeline.

## Deployment

```sh
cd apps/gatekeeper
bun run deploy     # bun run build && wrangler deploy
```

CI note: `terraform apply` and `wrangler deploy` are manual today. If you add a
GitHub Actions workflow, set `CLOUDFLARE_API_TOKEN` as a repository secret and
never print it.

## Legal

Serving the original game files from R2 — even gated behind coupons and hashes —
is still distributing copyrighted material. Only enable this for people you
authorize to have the files, and keep the manual "bring your own game" flow as
the default. This tooling does not change the project's copyright posture.
