.DEFAULT_GOAL := help

BASE_DIR := tmp/base
PATCH_DIR := tmp/patches
RELEASE_DIR := tmp/release
PUBLISH ?=
PATCHER ?=
CNC_DDRAW_DIR ?=
PATCHER_CNC_DDRAW_DIR := $(if $(strip $(CNC_DDRAW_DIR)),$(CNC_DDRAW_DIR),third_party/cnc-ddraw)
RESOURCE_FILES := strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat
GAME_FILES := Doraemon.exe $(RESOURCE_FILES)

ifeq ($(PUBLISH),1)
PATCH_DIR := patches
PATCH_DESTINATION := tracked
else
PATCH_DESTINATION := ignored candidate
endif

.PHONY: help setup build-patch build-patcher check-language check-publish check-patcher check-wrapper check-resources check-game check-payloads

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
	  '      Build a local Windows patcher with the vendored cnc-ddraw runtime.' \
	  '  make build-patch LANGUAGE=english PATCHER=1 CNC_DDRAW_DIR=/path/to/cnc-ddraw' \
	  '      Bundle your local cnc-ddraw files for the patcher’s Add graphics wrapper button.' \
	  '  make build-patcher' \
	  '      Build one configurable Windows patcher from whichever tracked patches/*.dmpatch files exist.' \
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

check-wrapper:
	@if [ -n "$(CNC_DDRAW_DIR)" ] && [ "$(PATCHER)" != 1 ]; then \
	  printf '%s\n' 'CNC_DDRAW_DIR is only used with PATCHER=1.'; exit 2; \
	fi
	@if [ "$(PATCHER)" = 1 ]; then \
	  missing=0; for file in ddraw.dll ddraw.ini 'cnc-ddraw config.exe'; do \
	    if [ ! -f "$(PATCHER_CNC_DDRAW_DIR)/$$file" ]; then \
	      printf '%s\n' "Missing $(PATCHER_CNC_DDRAW_DIR)/$$file. Choose a complete cnc-ddraw folder."; \
	      missing=1; \
	    fi; \
	  done; test $$missing -eq 0; \
	fi

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

build-patch: check-language check-publish check-patcher check-wrapper check-game
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
	  --output-dir "$(RELEASE_DIR)" \
	  --cnc-ddraw-dir "$(PATCHER_CNC_DDRAW_DIR)"
	@printf '%s\n' "Local Windows patcher written to $(RELEASE_DIR)/."
endif

build-patcher:
	@mkdir -p "$(RELEASE_DIR)"
	@set --; \
	if [ -f patches/english.dmpatch ]; then set -- "$$@" --english-payload patches/english.dmpatch; else printf '%s\n' 'English payload missing: English will be unavailable in the patcher.'; fi; \
	if [ -f patches/vietnamese.dmpatch ]; then set -- "$$@" --vietnamese-payload patches/vietnamese.dmpatch; else printf '%s\n' 'Vietnamese payload missing: Vietnamese will be unavailable in the patcher.'; fi; \
	if [ "$$#" -eq 0 ]; then printf '%s\n' 'No language payloads found in patches/. Nothing to build.'; exit 2; fi; \
	cargo run -p patch-build -- universal --output-dir "$(RELEASE_DIR)" --cnc-ddraw-dir "$(PATCHER_CNC_DDRAW_DIR)" "$$@"
