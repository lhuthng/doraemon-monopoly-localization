.DEFAULT_GOAL := help

BASE_DIR := workspace/base
PATCH_DIR := workspace/patches
RELEASE_DIR := workspace/release
PUBLISH ?=
PATCHER ?=
CNC_DDRAW_DIR ?=
LANGUAGE ?=
CONTRIBUTION ?=
SETUP ?= auto
PATCHER_CNC_DDRAW_DIR := $(if $(strip $(CNC_DDRAW_DIR)),$(CNC_DDRAW_DIR),vendor/cnc-ddraw)
RESOURCE_FILES := strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat voice.dat
GAME_FILES := Doraemon.exe $(RESOURCE_FILES)

ifeq ($(PUBLISH),1)
PATCH_DIR := content/patches
PATCH_DESTINATION := tracked
else
PATCH_DESTINATION := ignored candidate
endif

.PHONY: help setup dependencies check prepare ensure-studio import-contribution studio-en studio-vi build-dubbing build-sprites build-runtime build-patch build-patcher release translator-build translator-dev check-language check-setup check-publish check-patcher check-wrapper check-resources check-game check-payloads

help:
	@printf '%s\n' \
	  'Doraemon Monopoly localization toolkit' \
	  '' \
	  'Put your own untouched Cantonese game files in workspace/base/:' \
	  '  Doraemon.exe strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat voice.dat' \
	  '' \
	  'Recommended workflow:' \
	  '  1. Put private original game files in workspace/base/.' \
	  '  2. make setup' \
	  '  3. make studio-en or make studio-vi' \
	  '  4. make check' \
	  '  5. make build-patch LANGUAGE=<language> PUBLISH=1' \
	  '  6. make build-patcher' \
	  '' \
	  'Preparation and contributions:' \
	  '  make setup' \
	  '      Install Bun workspace dependencies, validate workspace/base, materialize local-game, and sync canonical content.' \
	  '  make prepare' \
	  '      Rebuild local-game workspaces from already-installed dependencies.' \
	  '  make import-contribution CONTRIBUTION=workspace/<contribution>.zip' \
	  '      Validate and merge a Translator Workshop ZIP into content/dubbing/.' \
	  '  make studio-en | make studio-vi' \
	  '      Reuse a complete workspace, prepare it only when missing, then launch Studio.' \
	  '      SETUP=1 always refreshes; SETUP=0 always skips preparation.' \
	  '' \
	  'Validation and builds:' \
	  '  make check' \
	  '      Run Rust workspace tests, shared package checks, and app checks.' \
	  '  make build-dubbing LANGUAGE=english PUBLISH=1' \
	  '      Build only content/patches/<language>/dubbing.dmpatch.' \
	  '  make build-sprites LANGUAGE=english PUBLISH=1' \
	  '      Build only the graphics component.' \
	  '  make build-runtime LANGUAGE=english PUBLISH=1' \
	  '      Build only the runtime component.' \
	  '  make build-patch LANGUAGE=english PUBLISH=1' \
	  '      Build all three components for one language.' \
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

setup: dependencies prepare

dependencies:
	@bun install --frozen-lockfile

check:
	@cargo test --workspace
	@cd apps/resource-studio && bun run check && bun test
	@cd apps/translator-workshop && bun run check && bun test

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

check-setup:
	@case "$(SETUP)" in auto|0|1) ;; *) printf '%s\n' 'SETUP must be auto (default), 0, or 1.'; exit 2 ;; esac

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
	@cd apps/resource-studio && bun run dubbing:sync english && bun run dubbing:sync vietnamese
	@printf '%s\n' 'Prepared private Studio workspaces. Start one with: cd apps/resource-studio && bun run dev-en'

ensure-studio: check-language check-setup
	@if [ "$(SETUP)" = 0 ]; then \
	  printf '%s\n' 'Skipping workspace preparation (SETUP=0).'; \
	elif [ "$(SETUP)" = 1 ]; then \
	  $(MAKE) setup; \
	else \
	  missing=0; \
	  for file in $(RESOURCE_FILES); do \
	    if [ ! -f "apps/resource-studio/local-game/$(LANGUAGE)/$$file" ]; then missing=1; fi; \
	  done; \
	  for file in strings.dat voice.dat; do \
	    if [ ! -f "apps/resource-studio/local-game/origin/$$file" ]; then missing=1; fi; \
	  done; \
	  if [ "$$missing" = 1 ]; then \
	    printf '%s\n' 'Studio workspace is missing or incomplete; running make setup.'; \
	    $(MAKE) setup; \
	  else \
	    printf '%s\n' 'Using the existing $(LANGUAGE) Studio workspace (SETUP=auto).'; \
	  fi; \
	fi

import-contribution:
	@test -n "$(CONTRIBUTION)" || { printf '%s\n' 'Usage: make import-contribution CONTRIBUTION=workspace/<contribution>.zip'; exit 2; }
	@cd apps/resource-studio && bun run dubbing:import -- "../../$(CONTRIBUTION)"

studio-en:
	@$(MAKE) ensure-studio LANGUAGE=english SETUP="$(SETUP)"
	@cd apps/resource-studio && bun run dev-en

studio-vi:
	@$(MAKE) ensure-studio LANGUAGE=vietnamese SETUP="$(SETUP)"
	@cd apps/resource-studio && bun run dev-vi

build-dubbing: check-language check-publish check-game
	@missing=0; for file in $(RESOURCE_FILES); do \
	  if [ ! -f "apps/resource-studio/local-game/$(LANGUAGE)/$$file" ]; then printf '%s\n' "Missing apps/resource-studio/local-game/$(LANGUAGE)/$$file. Run make prepare after preserving any local graphics edits."; missing=1; fi; \
	done; test $$missing -eq 0
	@cd apps/resource-studio && bun run dubbing:check $(LANGUAGE) && bun run dubbing:sync $(LANGUAGE)
	@mkdir -p "$(PATCH_DIR)/$(LANGUAGE)"
	cargo run -p patch-build -- release-parts \
	  --language "$(LANGUAGE)" \
	  --base-dir "$(BASE_DIR)" \
	  --target-dir "apps/resource-studio/local-game/$(LANGUAGE)" \
	  --output-dir "$(PATCH_DIR)/$(LANGUAGE)" \
	  --target dubbing

build-sprites: check-language check-publish check-game
	@mkdir -p "$(PATCH_DIR)/$(LANGUAGE)"
	cargo run -p patch-build -- release-parts --language "$(LANGUAGE)" --base-dir "$(BASE_DIR)" --target-dir "apps/resource-studio/local-game/$(LANGUAGE)" --output-dir "$(PATCH_DIR)/$(LANGUAGE)" --target sprites

build-runtime: check-language check-publish check-game
	@mkdir -p "$(PATCH_DIR)/$(LANGUAGE)"
	cargo run -p patch-build -- release-parts --language "$(LANGUAGE)" --base-dir "$(BASE_DIR)" --target-dir "apps/resource-studio/local-game/$(LANGUAGE)" --output-dir "$(PATCH_DIR)/$(LANGUAGE)" --target runtime --cnc-ddraw-dir "$(PATCHER_CNC_DDRAW_DIR)"

build-patch: build-dubbing build-sprites build-runtime

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
