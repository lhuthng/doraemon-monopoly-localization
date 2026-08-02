.DEFAULT_GOAL := help

BASE_DIR := workspace/base
PATCH_DIR := workspace/patches
RELEASE_DIR := workspace/release
PUBLISH ?=
PATCHER ?=
CNC_DDRAW_DIR ?=
LANGUAGE ?=
CONTRIBUTION ?=
PATCHER_CNC_DDRAW_DIR := $(if $(strip $(CNC_DDRAW_DIR)),$(CNC_DDRAW_DIR),vendor/cnc-ddraw)
RESOURCE_FILES := strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat voice.dat
GAME_FILES := Doraemon.exe $(RESOURCE_FILES)

ifeq ($(PUBLISH),1)
PATCH_DIR := content/patches
PATCH_DESTINATION := tracked
else
PATCH_DESTINATION := ignored candidate
endif

.PHONY: help dependencies check prepare fetch-base upload-base gatekeeper-mint gatekeeper-add-coupon gatekeeper-sync-coupons gatekeeper-list-coupons gatekeeper-delete-coupon apply-dubbing export-dubbing import-contribution studio-en studio-vi build-dubbing build-sprites build-runtime build-patch build-patcher release translator-build translator-dev check-language check-studio check-publish check-patcher check-wrapper check-resources check-game check-payloads

help:
	@printf '%s\n' \
	  'Doraemon Monopoly localization toolkit' \
	  '' \
	  'Put your own untouched Cantonese game files in workspace/base/:' \
	  '  Doraemon.exe strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat voice.dat' \
	  '' \
	  'Recommended workflow:' \
	  '  1. Put private original game files in workspace/base/.' \
	  '  2. make dependencies' \
	  '  3. make prepare' \
	  '  4. make apply-dubbing LANGUAGE=<language>' \
	  '  5. make studio-en or make studio-vi' \
	  '  6. make build-patch LANGUAGE=<language> PUBLISH=1' \
	  '  7. make build-patcher' \
	  '' \
	  'Preparation and contributions:' \
	  '  make dependencies' \
	  '      Install the locked Bun workspace dependencies.' \
	  '  make prepare' \
	  '      Rebuild Studio local-game from workspace/base and the current component patches only.' \
	  '  make fetch-base' \
	  '      Optional: fetch workspace/base files from the gatekeeper worker (needs CLOUDFLARE_GATEKEEPER_URL/SECRET).' \
	  '  make upload-base' \
	  '      Upload workspace/base files into the gatekeeper R2 bucket (needs R2_* env vars).' \
	  '  make gatekeeper-mint' \
	  '      Mint a coupon and print its SHA-256 for the gatekeeper worker.' \
	  '  make gatekeeper-add-coupon COUPON=...' \
	  '      Mint a coupon, record it, and push it live (needs CLOUDFLARE_API_TOKEN/ACCOUNT_ID in apps/gatekeeper/.env).' \
	  '  make gatekeeper-sync-coupons' \
	  '      Force-push the current active coupon set to Cloudflare.' \
	  '  make gatekeeper-list-coupons' \
	  '      List coupons and whether each is active or revoked.' \
	  '  make gatekeeper-delete-coupon COUPON=...|HASH=...' \
	  '      Revoke a coupon and push immediately.' \
	  '  make apply-dubbing LANGUAGE=english' \
	  '      Apply canonical content/dubbing/<language> to that prepared Studio workspace.' \
	  '  make import-contribution CONTRIBUTION=workspace/<contribution>.zip' \
	  '      Validate and merge a Translator Workshop ZIP into content/dubbing/.' \
	  '  make studio-en | make studio-vi' \
	  '      Launch Studio only; it never prepares or overwrites the workspace.' \
	  '' \
	  'Validation and builds:' \
	  '  make check' \
	  '      Run Rust workspace tests, shared package checks, and app checks.' \
	  '  make build-dubbing LANGUAGE=english PUBLISH=1' \
	  '      Export Studio dubbing to content/dubbing/, then build only the dubbing component.' \
	  '  make build-sprites LANGUAGE=english PUBLISH=1' \
	  '      Build only the graphics component.' \
	  '  make build-runtime LANGUAGE=english PUBLISH=1' \
	  '      Build only the runtime component.' \
	  '  make build-patch LANGUAGE=english PUBLISH=1' \
	  '      Export Studio dubbing to content/dubbing/, then build all three components.' \
	  '  make build-patcher' \
	  '      Embed tracked components into workspace/release/patcher.exe.' \
	  '  make release' \
	  '      Validate payload presence and build the local patcher artifact.' \
	  '' \
	  'PUBLISH=1 writes tracked content/patches/. Without it, output goes to ignored workspace/patches/.' \
	  '' \
	  'Source of truth: content/dubbing/ for dialogue and voices; Resource Studio local-game/ is generated.' \
	  '' \
	  'Run make help to see this workflow.'

dependencies:
	@bun install --frozen-lockfile

check:
	@cargo test --workspace
	@cd apps/resource-studio && bun run check && bun test
	@cd apps/translator-workshop && bun run check && bun test
	@cd apps/gatekeeper && bun run check && bun test

check-resources:
	@missing=0; for file in $(RESOURCE_FILES); do \
	  if [ ! -f "$(BASE_DIR)/$$file" ]; then printf '%s\n' "Missing $(BASE_DIR)/$$file. Copy your original game resources into $(BASE_DIR)/."; missing=1; fi; \
	done; test $$missing -eq 0

check-game: check-resources
	@missing=0; for file in $(GAME_FILES); do \
	  if [ ! -f "$(BASE_DIR)/$$file" ]; then printf '%s\n' "Missing $(BASE_DIR)/$$file. Copy your original game files into $(BASE_DIR)/."; missing=1; fi; \
	done; test $$missing -eq 0

check-language:
	@case "$(LANGUAGE)" in english|vietnamese) ;; *) printf '%s\n' 'Choose LANGUAGE=english or LANGUAGE=vietnamese.'; exit 2 ;; esac

check-publish:
	@case "$(PUBLISH)" in ''|1) ;; *) printf '%s\n' 'PUBLISH must be empty or 1.'; exit 2 ;; esac

check-patcher:
	@case "$(PATCHER)" in ''|1) ;; *) printf '%s\n' 'PATCHER must be empty or 1.'; exit 2 ;; esac

check-wrapper:
	@if [ -n "$(CNC_DDRAW_DIR)" ] && [ "$(PATCHER)" != 1 ]; then printf '%s\n' 'CNC_DDRAW_DIR is only used with PATCHER=1.'; exit 2; fi

check-payloads:
	@missing=0; for language in english vietnamese; do \
	  for component in dubbing sprites runtime; do if [ ! -f "content/patches/$$language/$$component.dmpatch" ]; then printf '%s\n' "Missing content/patches/$$language/$$component.dmpatch."; missing=1; fi; done; \
	done; test $$missing -eq 0

prepare: check-resources check-payloads
	@mkdir -p apps/resource-studio/local-game/origin
	@cp $(BASE_DIR)/strings.dat apps/resource-studio/local-game/origin/strings.dat
	@cp $(BASE_DIR)/voice.dat apps/resource-studio/local-game/origin/voice.dat
	@cargo run -p patch-build -- materialize-parts --parts-dir content/patches/english --base-dir $(BASE_DIR) --output-dir apps/resource-studio/local-game/english
	@cargo run -p patch-build -- materialize-parts --parts-dir content/patches/vietnamese --base-dir $(BASE_DIR) --output-dir apps/resource-studio/local-game/vietnamese
	@printf '%s\n' 'Prepared Studio workspaces from workspace/base and content/patches. Run make apply-dubbing LANGUAGE=<language> before editing dialogue or voices.'

fetch-base:
	@cd apps/resource-studio && bun run fetch-base

upload-base:
	@cd apps/resource-studio && bun run upload-base

gatekeeper-mint:
	@cd apps/gatekeeper && bun run mint-coupon

gatekeeper-add-coupon:
	@cd apps/gatekeeper && bun run add-coupon $(COUPON)

gatekeeper-sync-coupons:
	@cd apps/gatekeeper && bun run sync-coupon-hashes

gatekeeper-list-coupons:
	@cd apps/gatekeeper && bun run list-coupons

gatekeeper-delete-coupon:
	@cd apps/gatekeeper && bun run delete-coupon $(COUPON) $(HASH)

check-studio: check-language
	@missing=0; for file in $(RESOURCE_FILES); do \
	  if [ ! -f "apps/resource-studio/local-game/$(LANGUAGE)/$$file" ]; then printf '%s\n' "Missing apps/resource-studio/local-game/$(LANGUAGE)/$$file. Run make prepare."; missing=1; fi; \
	done; \
	for file in strings.dat voice.dat; do \
	  if [ ! -f "apps/resource-studio/local-game/origin/$$file" ]; then printf '%s\n' "Missing apps/resource-studio/local-game/origin/$$file. Run make prepare."; missing=1; fi; \
	done; test $$missing -eq 0

apply-dubbing: check-studio
	@cd apps/resource-studio && bun run dubbing:check $(LANGUAGE) && bun run dubbing:sync $(LANGUAGE)

export-dubbing: check-studio
	@cd apps/resource-studio && bun run dubbing:export $(LANGUAGE) && bun run dubbing:check $(LANGUAGE)

import-contribution:
	@test -n "$(CONTRIBUTION)" || { printf '%s\n' 'Usage: make import-contribution CONTRIBUTION=workspace/<contribution>.zip'; exit 2; }
	@cd apps/resource-studio && bun run dubbing:import -- "../../$(CONTRIBUTION)"

studio-en:
	@cd apps/resource-studio && bun run dev-en

studio-vi:
	@cd apps/resource-studio && bun run dev-vi

build-dubbing: check-publish check-game export-dubbing
	@mkdir -p "$(PATCH_DIR)/$(LANGUAGE)"
	cargo run -p patch-build -- release-parts \
	  --language "$(LANGUAGE)" \
	  --base-dir "$(BASE_DIR)" \
	  --target-dir "apps/resource-studio/local-game/$(LANGUAGE)" \
	  --output-dir "$(PATCH_DIR)/$(LANGUAGE)" \
	  --target dubbing

build-sprites: check-publish check-game check-studio
	@mkdir -p "$(PATCH_DIR)/$(LANGUAGE)"
	cargo run -p patch-build -- release-parts --language "$(LANGUAGE)" --base-dir "$(BASE_DIR)" --target-dir "apps/resource-studio/local-game/$(LANGUAGE)" --output-dir "$(PATCH_DIR)/$(LANGUAGE)" --target sprites

build-runtime: check-publish check-game check-studio
	@mkdir -p "$(PATCH_DIR)/$(LANGUAGE)"
	cargo run -p patch-build -- release-parts --language "$(LANGUAGE)" --base-dir "$(BASE_DIR)" --target-dir "apps/resource-studio/local-game/$(LANGUAGE)" --output-dir "$(PATCH_DIR)/$(LANGUAGE)" --target runtime --cnc-ddraw-dir "$(PATCHER_CNC_DDRAW_DIR)"

build-patch: check-language check-publish check-game check-studio
	@cd apps/resource-studio && bun run dubbing:export $(LANGUAGE) && bun run dubbing:check $(LANGUAGE)
	@mkdir -p "$(PATCH_DIR)/$(LANGUAGE)"
	cargo run -p patch-build -- release-parts --language "$(LANGUAGE)" --base-dir "$(BASE_DIR)" --target-dir "apps/resource-studio/local-game/$(LANGUAGE)" --output-dir "$(PATCH_DIR)/$(LANGUAGE)" --target dubbing
	cargo run -p patch-build -- release-parts --language "$(LANGUAGE)" --base-dir "$(BASE_DIR)" --target-dir "apps/resource-studio/local-game/$(LANGUAGE)" --output-dir "$(PATCH_DIR)/$(LANGUAGE)" --target sprites
	cargo run -p patch-build -- release-parts --language "$(LANGUAGE)" --base-dir "$(BASE_DIR)" --target-dir "apps/resource-studio/local-game/$(LANGUAGE)" --output-dir "$(PATCH_DIR)/$(LANGUAGE)" --target runtime --cnc-ddraw-dir "$(PATCHER_CNC_DDRAW_DIR)"

release: check-payloads build-patcher

translator-build:
	@cd apps/translator-workshop && bun run build
	@rm -rf workspace/contributor-kit && mkdir -p workspace && cp -R apps/translator-workshop/build workspace/contributor-kit

translator-dev:
	@cd apps/translator-workshop && bun run dev

build-patcher:
	@mkdir -p "$(RELEASE_DIR)"
	@set --; \
	if [ -d content/patches/english ]; then set -- "$$@" --english-payload-dir content/patches/english; else printf '%s\n' 'English components missing.'; fi; \
	if [ -d content/patches/vietnamese ]; then set -- "$$@" --vietnamese-payload-dir content/patches/vietnamese; else printf '%s\n' 'Vietnamese components missing.'; fi; \
	if [ "$$#" -eq 0 ]; then exit 2; fi; \
	cargo run -p patch-build -- universal --output-dir "$(RELEASE_DIR)" --cnc-ddraw-dir "$(PATCHER_CNC_DDRAW_DIR)" "$$@"
