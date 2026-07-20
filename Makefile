.DEFAULT_GOAL := help

BASE_DIR := tmp/base
PATCH_DIR := tmp/patches
RELEASE_DIR := tmp/release
PUBLISH ?=
PATCHER ?=
CNC_DDRAW_DIR ?=
LANGUAGE ?=
TARGET ?= all
PATCHER_CNC_DDRAW_DIR := $(if $(strip $(CNC_DDRAW_DIR)),$(CNC_DDRAW_DIR),third_party/cnc-ddraw)
RESOURCE_FILES := strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat voice.dat
GAME_FILES := Doraemon.exe $(RESOURCE_FILES)
VALID_LANGUAGES := english vietnamese
VALID_TARGETS := all doraemon nobita dorami shizuka suneo gian others sprites runtime

ifeq ($(PUBLISH),1)
PATCH_DIR := patches
PATCH_DESTINATION := tracked
else
PATCH_DESTINATION := ignored candidate
endif

# Every target belongs to its language directory. This keeps contributor
# payloads together whether one part or the complete set is being built.
LANG_DIR := $(PATCH_DIR)/$(LANGUAGE)

.PHONY: help setup build-patch build-patcher check-language check-target check-publish check-patcher check-wrapper check-resources check-game check-payloads

help:
	@printf '%s\n' \
	  'Doraemon Monopoly localization toolkit' \
	  '' \
	  'Put your own untouched Cantonese game files in tmp/base/:' \
	  '  Doraemon.exe strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat voice.dat' \
	  '' \
	  'Commands:' \
	  '  make setup' \
	  '      Materialize private English and Vietnamese Studio workspaces from tracked patches.' \
	  '  make build-patch LANGUAGE=english TARGET=all' \
	  '  make build-patch LANGUAGE=english TARGET=doraemon' \
	  '  make build-patch LANGUAGE=vietnamese TARGET=all' \
	  '      Create nine-part multipart payloads in tmp/patches/{language}/ for review.' \
	  '      TARGET may be: all, doraemon, nobita, dorami, shizuka, suneo, gian, others, sprites, runtime.' \
	  '      TARGET=all writes all nine files; single targets write only their own file.' \
	  '  make build-patch LANGUAGE=english TARGET=all PUBLISH=1' \
	  '      Write reviewed payloads directly to tracked patches/ for committing.' \
	  '  make build-patch LANGUAGE=english PATCHER=1' \
	  '      Build a local Windows patcher with the vendored cnc-ddraw runtime.' \
	  '  make build-patch LANGUAGE=english PATCHER=1 CNC_DDRAW_DIR=/path/to/cnc-ddraw' \
	  '      Bundle your local cnc-ddraw files for the patcher'\''s Add graphics wrapper button.' \
	  '  make build-patcher' \
	  '      Build one configurable Windows patcher from whichever tracked patches/*.dmpatch files exist.' \
	  '' \
	  'Target ownership:' \
	  '  loc-doraemon.dmpatch : strings group 003 (Doraemon dialogues + voice)' \
	  '  loc-nobita.dmpatch   : strings group 004 (Nobita dialogues + voice)' \
	  '  loc-dorami.dmpatch   : strings group 005 (Dorami dialogues + voice)' \
	  '  loc-shizuka.dmpatch  : strings group 006 (Shizuka dialogues + voice)' \
	  '  loc-suneo.dmpatch    : strings group 007 (Suneo dialogues + voice)' \
	  '  loc-gian.dmpatch     : strings group 008 (Gian dialogues + voice)' \
	  '  loc-others.dmpatch   : groups 000,001,002 + shared action text/voice + menu/misc voice' \
	  '  sprites.dmpatch      : sysfont.dat, Sprite1.dat, sprite2.dat, bitmaps.dat deltas' \
	  '  runtime.dmpatch      : Doraemon.exe changes, bundled cnc-ddraw + doraudio.dll' \
	  '' \
	  'Tracked: patches/*.dmpatch patches/*/ (shareable resource changes only)' \
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

check-target:
	@case "$(TARGET)" in \
	  all|doraemon|nobita|dorami|shizuka|suneo|gian|others|sprites|runtime) ;; \
	  *) printf '%s\n' 'Choose TARGET=all|doraemon|nobita|dorami|shizuka|suneo|gian|others|sprites|runtime. Run make help for details.'; exit 2 ;; \
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
	  dir="patches/$$language"; \
	  if [ ! -d "$$dir" ]; then \
	    printf '%s\n' "Missing patches/$$language/ directory. This language cannot be materialized yet."; \
	    missing=1; \
	  fi; \
	  for target in doraemon nobita dorami shizuka suneo gian others sprites runtime; do \
	    if [ ! -f "$$dir/loc-$$target.dmpatch" ] && [ ! -f "$$dir/$$target.dmpatch" ]; then \
	      printf '%s\n' "  Missing $$dir/loc-$$target.dmpatch or $$dir/$$target.dmpatch"; \
	      missing=1; \
	    fi; \
	  done; \
	done; test $$missing -eq 0

setup: check-resources check-payloads
	@cd resource-studio && bun run setup-en ../$(BASE_DIR)
	@cd resource-studio && bun run setup-vi ../$(BASE_DIR)
	@printf '%s\n' 'Prepared private Studio workspaces. Start one with: cd resource-studio && bun run dev-en'

build-patch: check-language check-target check-publish check-patcher check-wrapper check-game
	@missing=0; for file in $(RESOURCE_FILES); do \
	  if [ ! -f "resource-studio/local-game/$(LANGUAGE)/$$file" ]; then \
	    printf '%s\n' "Missing resource-studio/local-game/$(LANGUAGE)/$$file. Run make setup or prepare your workspace first."; \
	    missing=1; \
	  fi; \
	done; test $$missing -eq 0
	@mkdir -p "$(LANG_DIR)"
	cargo run -p patch-build -- release-parts \
	  --language "$(LANGUAGE)" \
	  --base-dir "$(BASE_DIR)" \
	  --target-dir "resource-studio/local-game/$(LANGUAGE)" \
	  --output-dir "$(LANG_DIR)" \
	  --target "$(TARGET)" \
	  --payload-only
	@if [ "$(TARGET)" = "all" ]; then \
	  printf '%s\n' "Wrote $(PATCH_DESTINATION) multipart payloads to $(LANG_DIR)/."; \
	  for t in doraemon nobita dorami shizuka suneo gian others sprites runtime; do \
	    f="loc-$$t.dmpatch"; \
	    [ -f "$(LANG_DIR)/$$f" ] && printf '  %s\n' "$$f"; \
	  done; \
	  f="sprites.dmpatch"; \
	  [ -f "$(LANG_DIR)/$$f" ] && printf '  %s\n' "$$f"; \
	  f="runtime.dmpatch"; \
	  [ -f "$(LANG_DIR)/$$f" ] && printf '  %s\n' "$$f"; \
	else \
	  printf '%s\n' "Wrote $(PATCH_DESTINATION) payload: $(LANG_DIR)/$(TARGET).dmpatch"; \
	fi
ifeq ($(PUBLISH),1)
	@printf '%s\n' "Review the diff, then commit."
else
	@printf '%s\n' "Review it, then rerun with PUBLISH=1 to write tracked payloads."
endif

ifeq ($(PATCHER),1)
	@mkdir -p "$(RELEASE_DIR)"
	cargo run -p patch-build -- package \
	  --payload "$(LANG_DIR)/$(LANGUAGE).dmpatch" \
	  --output-dir "$(RELEASE_DIR)" \
	  --cnc-ddraw-dir "$(PATCHER_CNC_DDRAW_DIR)"
	@printf '%s\n' "Local Windows patcher written to $(RELEASE_DIR)/."
endif

build-patcher:
	@mkdir -p "$(RELEASE_DIR)"
	@set --; \
	if [ -d patches/english ]; then set -- "$$@" --english-payload-dir patches/english; else printf '%s\n' 'English payload missing: English will be unavailable in the patcher.'; fi; \
	if [ -d patches/vietnamese ]; then set -- "$$@" --vietnamese-payload-dir patches/vietnamese; else printf '%s\n' 'Vietnamese payload missing: Vietnamese will be unavailable in the patcher.'; fi; \
	if [ "$$#" -eq 0 ]; then printf '%s\n' 'No language payloads found in patches/. Nothing to build.'; exit 2; fi; \
	cargo run -p patch-build -- universal --output-dir "$(RELEASE_DIR)" --cnc-ddraw-dir "$(PATCHER_CNC_DDRAW_DIR)" "$$@"
