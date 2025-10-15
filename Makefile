ARGS ?=

.PHONY: help
help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@echo '  dev: Start development environment with two nodes in tmux'
	@echo '  check: Check all Rust code'
	@echo '  install: Install dependencies'
	@echo '  build: Build all Rust packages'
	@echo '  test: Run all tests'
	@echo '  lint: Run clippy linter'
	@echo '  fmt: Format code'
	@echo '  fmt-check: Check code formatting'
	@echo '  clean: Clean build artifacts'

.PHONY: dev
dev: ## Start development environment with two nodes in tmux
	./bin/dev.sh

.PHONY: check
check: ## Check all Rust code
	cargo check --all

.PHONY: install
install: ## Install dependencies
	cargo fetch

.PHONY: build
build: ## Build all Rust packages
	cargo build --all

.PHONY: test
test: ## Run all tests
	cargo test --all

.PHONY: lint
lint: ## Run clippy linter
	cargo clippy --all -- -D warnings

.PHONY: fmt
fmt: ## Format code
	cargo fmt --all

.PHONY: fmt-check
fmt-check: ## Check code formatting
	cargo fmt --all -- --check

.PHONY: types
types: ## Run type checking (alias for check)
	@$(MAKE) check

.PHONY: clean
clean: ## Clean build artifacts
	cargo clean