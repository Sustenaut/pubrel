# The shared targets (venv, lock, fmt, lint, test, doc, build, functional)
# come from pubkit.mk, which pubkit manages; this file adds pubrel's own.
# SUPPLY-CHAIN.md is written for these crates by pubkit.mk's
# `make supply-chain`, and checked by `make supply-chain-check`.
SUPPLY_CHAIN_CRATES := pubrel

include pubkit.mk

.DEFAULT_GOAL := help
export PUBREL_BIN_DIR ?= $(CURDIR)/target/debug
# The released pub the suite drives; override to point elsewhere.
export PUB_BIN_DIR ?= $(dir $(shell command -v pub 2>/dev/null || echo $(HOME)/.cargo/bin/pub))

.PHONY: check clean fuzz-smoke deny vet publish-check msrv

check: sync-check fmt-check lint test doc functional publish-check supply-chain-check vet deny ## Everything CI runs

clean: ## Remove build artifacts
	cargo clean
	rm -rf .pytest_cache tests/.pytest_cache

# What `pubrel check` reads from a pull request, fuzzed (fuzz/).
FUZZ_TARGETS := release_json unreleased_json corpus_lock manifest
FUZZ_SECONDS ?= 60

fuzz-smoke: ## Replay fuzz regressions, then fuzz each target briefly (needs nightly and cargo-fuzz)
	cd fuzz && cargo +nightly fuzz build
	cd fuzz && for target in $(FUZZ_TARGETS); do \
	  mkdir -p corpus/$$target regressions/$$target; \
	  cargo +nightly fuzz run $$target corpus/$$target regressions/$$target -- \
	    -max_total_time=$(FUZZ_SECONDS) -timeout=10 || exit 1; \
	done

# The verify build gets a target directory of its own. Sharing target/
# leaves dep-info naming the packaged copy's sources, after which cargo
# reports pubrel Fresh however src/ changes, and tests run a stale binary.
publish-check: ## Package pubrel and verify it builds as published
	CARGO_TARGET_DIR=$(CURDIR)/target/publish-check $(CARGO) publish --dry-run --locked

vet: ## Every dependency is audited, trusted, or exempt with its evidence
	cargo vet --locked

deny: ## Licence and advisory audit
	$(CARGO) deny --locked check

# The oldest Rust pubrel promises: its rust-version. CI passes MSRV.
MSRV ?= $(shell sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml)

msrv: ## Build on the minimum Rust version (rustup toolchain install $(MSRV))
	$(CARGO) +$(MSRV) check --locked
