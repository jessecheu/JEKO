.DEFAULT_GOAL := help

NIX := nix develop --command

INSTANCE := $(or $(JEKO_INSTANCE),$(notdir $(CURDIR)))

.PHONY: help info run build release check fmt lint test gen-content gen-monsters validate shell nix-build clean

help:
	@echo "jeko [$(INSTANCE)] — make targets:"
	@echo "  make info         which checkout this is: folder, branch, commit, .env, build dir"
	@echo "  make run          run the game (cargo run, debug build)"
	@echo "  make build        debug build"
	@echo "  make release      optimized release build"
	@echo "  make check        cargo check (fast type-check, no codegen)"
	@echo "  make fmt          cargo fmt"
	@echo "  make lint         cargo clippy --all-targets"
	@echo "  make test         run the unit tests (cargo test)"
	@echo "  make gen-content  regenerate assets/dialogue.json via Kimi K3.1 (needs CEREBRAS_API_KEY in .env)"
	@echo "  make gen-monsters N=8  design N monsters into content/monsters/gen-*.json (needs a key)"
	@echo "  make validate     check assets/dialogue.json and the content dir the way the game loads them (offline)"
	@echo "  make shell        drop into the nix dev shell"
	@echo "  make nix-build    build the release binary as a nix derivation (needs Cargo.lock)"
	@echo "  make clean        remove target/"

info:
	@echo "instance  $(INSTANCE)"
	@echo "folder    $(CURDIR)"
	@echo "branch    $$(git rev-parse --abbrev-ref HEAD 2>/dev/null) @ $$(git rev-parse --short HEAD 2>/dev/null)$$(git diff --quiet 2>/dev/null || echo ' (uncommitted changes)')"
	@echo "build dir $${CARGO_TARGET_DIR:-$(CURDIR)/target}"
	@if [ -f .env ]; then grep -q '^CEREBRAS_API_KEY=..' .env && echo ".env      present, API key set (live questions)" || echo ".env      present, no API key (offline fallback bank)"; else echo ".env      missing (offline fallback bank) — cp .env.example .env"; fi

run:
	$(NIX) cargo run

build:
	$(NIX) cargo build

release:
	$(NIX) cargo build --release

check:
	$(NIX) cargo check

fmt:
	$(NIX) cargo fmt

lint:
	$(NIX) cargo clippy --all-targets

test:
	$(NIX) cargo test

gen-content:
	$(NIX) cargo run --bin gen_content

# make gen-monsters N=8: design N monsters into content/monsters/gen-*.json
gen-monsters:
	$(NIX) cargo run --bin gen_content -- --monsters $(or $(N),4)

# Offline, no key: exits non-zero if the dialogue bank or any content file is rejected.
validate:
	$(NIX) cargo run --quiet --bin validate_content

shell:
	nix develop

nix-build:
	nix build

clean:
	rm -rf target result
