.DEFAULT_GOAL := help

BASE_DIR := tmp/base
PATCH_DIR := tmp/patches
RELEASE_DIR := tmp/release
PUBLISH ?=
PATCHER ?=
CNC_DDRAW_DIR ?=
LANGUAGE ?=
CONTRIBUTION ?=
PATCHER_CNC_DDRAW_DIR := $(if $(strip $(CNC_DDRAW_DIR)),$(CNC_DDRAW_DIR),third_party/cnc-ddraw)
RESOURCE_FILES := strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat voice.dat
GAME_FILES := Doraemon.exe $(RESOURCE_FILES)

ifeq ($(PUBLISH),1)
PATCH_DIR := patches
PATCH_DESTINATION := tracked
else
PATCH_DESTINATION := ignored candidate
endif

.PHONY: help check setup import-contribution studio-en studio-vi build-dubbing build-sprites build-runtime build-patch build-patcher release translator-build translator-dev check-language check-publish check-patcher check-wrapper check-resources check-game check-payloads

help:
	@printf '%s\n' \
	  'Doraemon Monopoly localization toolkit' \
	  '' \
	  'Put your own untouched Cantonese game files in tmp/base/:' \
	  '  Doraemon.exe strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat voice.dat' \
	  '' \
	  'Recommended workflow:' \
	  '  1. Put private original game files in tmp/base/.' \
	  '  2. make setup' \
	  '  3. make studio-en or make studio-vi' \
	  '  4. make check' \
	  '  5. make build-patch LANGUAGE=<language> PUBLISH=1' \
	  '  6. make build-patcher' \
	  '' \
	  'Preparation and contributions:' \
	  '  make setup' \
	  '      Generate ignored local-game workspaces from tmp/base and patches/.' \
	  '  make import-contribution CONTRIBUTION=tmp/<contribution>.zip' \
	  '      Validate and merge a Translator Workshop ZIP into dubbing/.' \
	  '  make studio-en | make studio-vi' \
	  '      Prepare and launch the matching Resource Studio workspace.' \
	  '' \
	  'Validation and builds:' \
	  '  make check' \
	  '      Run Rust workspace tests and Resource Studio checks/tests.' \
	  '  make build-dubbing LANGUAGE=english PUBLISH=1' \
	  '      Build only patches/<language>/dubbing.dmpatch.' \
	  '  make build-sprites LANGUAGE=english PUBLISH=1' \
	  '      Build only the graphics component.' \
	  '  make build-runtime LANGUAGE=english PUBLISH=1' \
	  '      Build only the runtime component.' \
	  '  make build-patch LANGUAGE=english PUBLISH=1' \
	  '      Build all three components for one language.' \
	  '  make build-patcher' \
	  '      Embed tracked components into tmp/release/patcher.exe.' \
	  '  make release' \
	  '      Validate payload presence and build the local patcher artifact.' \
	  '' \
	  'PUBLISH=1 writes tracked patches/. Without it, output goes to ignored tmp/patches/.' \
	  '' \
	  'Source of truth: dubbing/ for dialogue and voices; Resource Studio local-game/ is generated.' \
	  '' \
	  'Run make help to see this workflow.'

check:
	@cargo test --workspace
	@cd resource-studio && bun run check && bun test

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
	  for component in dubbing sprites runtime; do if [ ! -f "patches/$$language/$$component.dmpatch" ]; then printf '%s\n' "Missing patches/$$language/$$component.dmpatch."; missing=1; fi; done; \
	done; test $$missing -eq 0

setup: check-resources check-payloads
	@cargo run -p patch-build -- materialize-parts --parts-dir patches/english --base-dir $(BASE_DIR) --output-dir resource-studio/local-game/english
	@cargo run -p patch-build -- materialize-parts --parts-dir patches/vietnamese --base-dir $(BASE_DIR) --output-dir resource-studio/local-game/vietnamese
	@cd resource-studio && bun run dubbing:sync english && bun run dubbing:sync vietnamese
	@printf '%s\n' 'Prepared private Studio workspaces. Start one with: cd resource-studio && bun run dev-en'

import-contribution:
	@test -n "$(CONTRIBUTION)" || { printf '%s\n' 'Usage: make import-contribution CONTRIBUTION=tmp/<contribution>.zip'; exit 2; }
	@cd resource-studio && bun run dubbing:import -- "../$(CONTRIBUTION)"

studio-en: setup
	@cd resource-studio && bun run dev-en

studio-vi: setup
	@cd resource-studio && bun run dev-vi

build-dubbing: check-language check-publish check-game
	@missing=0; for file in $(RESOURCE_FILES); do \
	  if [ ! -f "resource-studio/local-game/$(LANGUAGE)/$$file" ]; then printf '%s\n' "Missing resource-studio/local-game/$(LANGUAGE)/$$file. Run make setup after preserving any local graphics edits."; missing=1; fi; \
	done; test $$missing -eq 0
	@cd resource-studio && bun run dubbing:check $(LANGUAGE) && bun run dubbing:sync $(LANGUAGE)
	@mkdir -p "$(PATCH_DIR)/$(LANGUAGE)"
	cargo run -p patch-build -- release-parts \
	  --language "$(LANGUAGE)" \
	  --base-dir "$(BASE_DIR)" \
	  --target-dir "resource-studio/local-game/$(LANGUAGE)" \
	  --output-dir "$(PATCH_DIR)/$(LANGUAGE)" \
	  --target dubbing

build-sprites: check-language check-publish check-game
	@mkdir -p "$(PATCH_DIR)/$(LANGUAGE)"
	cargo run -p patch-build -- release-parts --language "$(LANGUAGE)" --base-dir "$(BASE_DIR)" --target-dir "resource-studio/local-game/$(LANGUAGE)" --output-dir "$(PATCH_DIR)/$(LANGUAGE)" --target sprites

build-runtime: check-language check-publish check-game
	@mkdir -p "$(PATCH_DIR)/$(LANGUAGE)"
	cargo run -p patch-build -- release-parts --language "$(LANGUAGE)" --base-dir "$(BASE_DIR)" --target-dir "resource-studio/local-game/$(LANGUAGE)" --output-dir "$(PATCH_DIR)/$(LANGUAGE)" --target runtime --cnc-ddraw-dir "$(PATCHER_CNC_DDRAW_DIR)"

build-patch: build-dubbing build-sprites build-runtime

release: check-payloads build-patcher

translator-build:
	@cd translator-site && bun run build
	@rm -rf tmp/contributor-kit && mkdir -p tmp && cp -R translator-site/build tmp/contributor-kit

translator-dev:
	@cd translator-site && bun run dev

build-patcher:
	@mkdir -p "$(RELEASE_DIR)"
	@set --; \
	if [ -d patches/english ]; then set -- "$$@" --english-payload-dir patches/english; else printf '%s\n' 'English components missing.'; fi; \
	if [ -d patches/vietnamese ]; then set -- "$$@" --vietnamese-payload-dir patches/vietnamese; else printf '%s\n' 'Vietnamese components missing.'; fi; \
	if [ "$$#" -eq 0 ]; then exit 2; fi; \
	cargo run -p patch-build -- universal --output-dir "$(RELEASE_DIR)" --cnc-ddraw-dir "$(PATCHER_CNC_DDRAW_DIR)" "$$@"
