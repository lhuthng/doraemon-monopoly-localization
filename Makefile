.DEFAULT_GOAL := help

BASE_DIR := tmp/base
PATCH_DIR := tmp/patches
RELEASE_DIR := tmp/release
PUBLISH ?=
PATCHER ?=
RESOURCE_FILES := strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat
GAME_FILES := Doraemon.exe $(RESOURCE_FILES)

ifeq ($(PUBLISH),1)
PATCH_DIR := patches
PATCH_DESTINATION := tracked
else
PATCH_DESTINATION := ignored candidate
endif

.PHONY: help setup build-patch check-language check-publish check-patcher check-resources check-game check-payloads

help:
	@printf '%s\n' \
	  'Doraemon Monopoly localization toolkit' \
	  '' \
	  'Put your own untouched Cantonese game files in tmp/base/:' \
	  '  Doraemon.exe strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat' \
	  '' \
	  'Commands:' \
	  '  make setup' \
	  '      Materialize private English and Vietnamese Studio workspaces from tracked patches.' \
	  '  make build-patch LANGUAGE=english' \
	  '  make build-patch LANGUAGE=vietnamese' \
	  '      Create an ignored candidate payload in tmp/patches/ for review.' \
	  '  make build-patch LANGUAGE=english PUBLISH=1' \
	  '      Write the reviewed payload directly to tracked patches/ for committing.' \
	  '  make build-patch LANGUAGE=english PATCHER=1' \
	  '      Also build a local Windows patcher EXE in tmp/release/.' \
	  '' \
	  'Tracked: patches/*.dmpatch (shareable resource changes only)' \
	  'Ignored: tmp/base/ (your game), resource-studio/local-game/, tmp/patches/, tmp/release/'

check-resources:
	@missing=0; for file in $(RESOURCE_FILES); do \
	  if [ ! -f "$(BASE_DIR)/$$file" ]; then \
	    printf '%s\n' "Missing $(BASE_DIR)/$$file. Copy your original game resources into $(BASE_DIR)/."; \
	    missing=1; \
	  fi; \
	done; test $$missing -eq 0

check-language:
	@case "$(LANGUAGE)" in \
	  english|vietnamese) ;; \
	  *) printf '%s\n' 'Choose LANGUAGE=english or LANGUAGE=vietnamese. Run make help for details.'; exit 2 ;; \
	esac

check-publish:
	@case "$(PUBLISH)" in \
	  ''|1) ;; \
	  *) printf '%s\n' 'PUBLISH must be empty or 1. Use PUBLISH=1 to write directly to patches/.'; exit 2 ;; \
	esac

check-patcher:
	@case "$(PATCHER)" in \
	  ''|1) ;; \
	  *) printf '%s\n' 'PATCHER must be empty or 1. Use PATCHER=1 to build a local Windows EXE.'; exit 2 ;; \
	esac

check-game:
	@missing=0; for file in $(GAME_FILES); do \
	  if [ ! -f "$(BASE_DIR)/$$file" ]; then \
	    printf '%s\n' "Missing $(BASE_DIR)/$$file. Copy your original game files into $(BASE_DIR)/."; \
	    missing=1; \
	  fi; \
	done; test $$missing -eq 0

check-payloads:
	@missing=0; for language in english vietnamese; do \
	  if [ ! -f "patches/$$language.dmpatch" ]; then \
	    printf '%s\n' "Missing patches/$$language.dmpatch. This language cannot be materialized yet."; \
	    missing=1; \
	  fi; \
	done; test $$missing -eq 0

setup: check-resources check-payloads
	@cd resource-studio && bun run setup-en ../$(BASE_DIR)
	@cd resource-studio && bun run setup-vi ../$(BASE_DIR)
	@printf '%s\n' 'Prepared private Studio workspaces. Start one with: cd resource-studio && bun run dev-en'

build-patch: check-language check-publish check-patcher check-game
	@missing=0; for file in $(RESOURCE_FILES); do \
	  if [ ! -f "resource-studio/local-game/$(LANGUAGE)/$$file" ]; then \
	    printf '%s\n' "Missing resource-studio/local-game/$(LANGUAGE)/$$file. Run make setup or prepare your workspace first."; \
	    missing=1; \
	  fi; \
	done; test $$missing -eq 0
	@mkdir -p "$(PATCH_DIR)"
	cargo run -p patch-build -- release \
	  --language "$(LANGUAGE)" \
	  --base-dir "$(BASE_DIR)" \
	  --target-dir "resource-studio/local-game/$(LANGUAGE)" \
	  --output-dir "$(PATCH_DIR)" \
	  --payload-only
	@printf '%s\n' "Wrote $(PATCH_DESTINATION) payload: $(PATCH_DIR)/$(LANGUAGE).dmpatch"
ifeq ($(PUBLISH),1)
	@printf '%s\n' "Review the diff, then commit patches/$(LANGUAGE).dmpatch."
else
	@printf '%s\n' "Review it, then rerun with PUBLISH=1 to write patches/$(LANGUAGE).dmpatch."
endif
ifeq ($(PATCHER),1)
	@mkdir -p "$(RELEASE_DIR)"
	cargo run -p patch-build -- package \
	  --payload "$(PATCH_DIR)/$(LANGUAGE).dmpatch" \
	  --output-dir "$(RELEASE_DIR)"
	@printf '%s\n' "Local Windows patcher written to $(RELEASE_DIR)/."
endif
