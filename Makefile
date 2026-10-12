# The shared targets come from pubkit.mk, which pubkit manages: the ones the
# Rust crate standard requires (`make required` lists what CI runs), and
# venv, lock, fmt, test, build and supply-chain. This file adds pubrel's own.
SUPPLY_CHAIN_CRATES := pubrel

include pubkit.mk

.DEFAULT_GOAL := help
export PUBREL_BIN_DIR ?= $(CURDIR)/target/debug
# The released pub the suite drives; override to point elsewhere.
export PUB_BIN_DIR ?= $(dir $(shell command -v pub 2>/dev/null || echo $(HOME)/.cargo/bin/pub))

.PHONY: check clean

check: sync-check fmt-check lint test doc functional publish-check supply-chain-check vet deny ## Everything CI runs

clean: ## Remove build artifacts
	cargo clean
	rm -rf .pytest_cache tests/.pytest_cache
