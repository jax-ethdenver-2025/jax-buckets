ARGS ?=

# TODO: Add more projects as needed here
PROJECTS := rust

.PHONY: help
help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Run-all targets (operate on all projects):'
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*?##/ && !/^[a-zA-Z_-]+-%:/ { printf "  %-20s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@echo ''
	@echo 'Project-specific targets (use with -<project> suffix, e.g., -ts):'
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+-%:.*?##/ { gsub(/-%/, "-<project>", $$1); printf "  %-20s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@echo ''
	@echo 'Available projects:'
	@for project in $(PROJECTS); do \
		echo "  - $$project"; \
	done

# run a make command in the given directory
run-for:
	@if [ -d "./$(PROJECT)" ]; then \
		if [ -f "./$(PROJECT)/Makefile" ]; then \
			cd ./$(PROJECT) && make $(CMD); \
		else \
			echo "Error: Makefile not found in ./$(PROJECT) directory"; \
			exit 1; \
		fi; \
	else \
		echo "Error: Directory ./$(PROJECT) does not exist"; \
		exit 1; \
	fi

.PHONY: check
check: ## Check all projects
	@for project in $(PROJECTS); do \
		$(MAKE) run-for PROJECT=$$project CMD=check; \
	done

.PHONY: install
install: ## Install dependencies for all projects
	@for project in $(PROJECTS); do \
		$(MAKE) run-for PROJECT=$$project CMD=install; \
	done

.PHONY: check-%
check-%: ## Check the given project
	@$(MAKE) run-for PROJECT=$(@:check-%=%) CMD=check


.PHONY: install-%
install-%: ## Install dependencies for the given project
	@$(MAKE) run-for PROJECT=$(@:install-%=%) CMD=install

.PHONY: dev-%
dev-%: ## Run development server for the given project
	@$(MAKE) run-for PROJECT=$(@:dev-%=%) CMD=dev

.PHONY: build
build: ## Build all projects
	@for project in $(PROJECTS); do \
		$(MAKE) run-for PROJECT=$$project CMD=build; \
	done

.PHONY: build-%
build-%: ## Build the given project
	@$(MAKE) run-for PROJECT=$(@:build-%=%) CMD=build

.PHONY: test
test: ## Run tests for all projects
	@for project in $(PROJECTS); do \
		$(MAKE) run-for PROJECT=$$project CMD=test; \
	done

.PHONY: test-%
test-%: ## Run tests for the given project
	@$(MAKE) run-for PROJECT=$(@:test-%=%) CMD=test

.PHONY: lint
lint: ## Run linters for all projects
	@for project in $(PROJECTS); do \
		$(MAKE) run-for PROJECT=$$project CMD=lint; \
	done

.PHONY: lint-%
lint-%: ## Run linters for the given project
	@$(MAKE) run-for PROJECT=$(@:lint-%=%) CMD=lint

.PHONY: fmt
fmt: ## Format all projects
	@for project in $(PROJECTS); do \
		$(MAKE) run-for PROJECT=$$project CMD=fmt; \
	done

.PHONY: fmt-%
fmt-%: ## Format the given project
	@$(MAKE) run-for PROJECT=$(@:fmt-%=%) CMD=fmt

.PHONY: fmt-check
fmt-check: ## Check formatting for all projects
	@for project in $(PROJECTS); do \
		$(MAKE) run-for PROJECT=$$project CMD=fmt-check; \
	done

.PHONY: fmt-check-%
fmt-check-%: ## Check formatting for the given project
	@$(MAKE) run-for PROJECT=$(@:fmt-check-%=%) CMD=fmt-check

.PHONY: types
types: ## Run type checking for all projects
	@for project in $(PROJECTS); do \
		$(MAKE) run-for PROJECT=$$project CMD=types; \
	done

.PHONY: types-%
types-%: ## Run type checking for the given project
	@$(MAKE) run-for PROJECT=$(@:types-%=%) CMD=types

.PHONY: clean
clean: ## Clean all projects
	@for project in $(PROJECTS); do \
		$(MAKE) run-for PROJECT=$$project CMD=clean; \
	done

.PHONY: clean-%
clean-%: ## Clean the given project
	@$(MAKE) run-for PROJECT=$(@:clean-%=%) CMD=clean