.DEFAULT_GOAL := help

NIX := nix develop --command

.PHONY: help run build release check fmt lint test gen-content shell nix-build clean

help:
	@echo "jeko — make targets:"
	@echo "  make run          run the game (cargo run, debug build)"
	@echo "  make build        debug build"
	@echo "  make release      optimized release build"
	@echo "  make check        cargo check (fast type-check, no codegen)"
	@echo "  make fmt          cargo fmt"
	@echo "  make lint         cargo clippy --all-targets"
	@echo "  make test         run the unit tests (cargo test)"
	@echo "  make gen-content  regenerate assets/dialogue.json via Kimi K3.1 (needs CEREBRAS_API_KEY in .env)"
	@echo "  make shell        drop into the nix dev shell"
	@echo "  make nix-build    build the release binary as a nix derivation (needs Cargo.lock)"
	@echo "  make clean        remove target/"

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

shell:
	nix develop

nix-build:
	nix build

clean:
	rm -rf target result
