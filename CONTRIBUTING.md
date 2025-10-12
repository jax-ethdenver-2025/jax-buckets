# Contributing to JaxBucket

Thank you for your interest in contributing to JaxBucket! This document provides guidelines for contributing to the project.

## Getting Started

Before contributing, please:

1. Read the [README.md](README.md) to understand the project
2. Review [DEVELOPMENT.md](DEVELOPMENT.md) for setup instructions
3. Check [existing issues](https://github.com/jax-ethdenver-2025/jax-bucket/issues) for open tasks

## Ways to Contribute

### Reporting Bugs

If you find a bug, please open an issue with:
- **Clear title** describing the problem
- **Steps to reproduce** the bug
- **Expected behavior** vs actual behavior
- **Environment details** (OS, Rust version, etc.)
- **Logs** if applicable (use `RUST_LOG=debug`)

### Suggesting Features

Feature requests are welcome! Please open an issue with:
- **Use case** - Why is this feature needed?
- **Proposed solution** - How should it work?
- **Alternatives** - Other approaches you considered
- **Impact** - Who would benefit from this?

### Contributing Code

We welcome code contributions! Here's the process:

## Contribution Workflow

### 1. Fork and Clone

```bash
# Fork the repo on GitHub, then clone your fork
git clone https://github.com/YOUR_USERNAME/jax-bucket.git
cd jax-bucket
```

### 2. Create a Branch

Use descriptive branch names:

```bash
# Feature branches
git checkout -b feature/add-conflict-resolution

# Bug fix branches
git checkout -b fix/sync-race-condition

# Documentation branches
git checkout -b docs/improve-api-docs
```

### 3. Make Changes

- Follow the [code style guide](#code-style)
- Add tests for new functionality
- Update documentation as needed
- Keep commits focused and atomic

### 4. Commit Your Changes

We use **conventional commits** for clear history and automated changelog generation:

```bash
# Format: <type>: <description>

# Types:
git commit -m "feat: add streaming encryption for large files"
git commit -m "fix: prevent race condition in sync manager"
git commit -m "docs: add examples for bucket sharing"
git commit -m "refactor: simplify manifest serialization"
git commit -m "test: add integration tests for P2P sync"
git commit -m "chore: update dependencies"
git commit -m "perf: optimize blob storage"
```

**Commit types:**
- `feat:` - New feature (minor version bump)
- `fix:` - Bug fix (patch version bump)
- `docs:` - Documentation only
- `refactor:` - Code restructuring (no behavior change)
- `test:` - Adding or updating tests
- `chore:` - Maintenance tasks
- `perf:` - Performance improvements

**Breaking changes:**
```bash
git commit -m "feat!: change manifest format to support versioning"

# Or in commit body:
git commit -m "feat: redesign sync protocol

BREAKING CHANGE: sync protocol v2 is incompatible with v1"
```

### 5. Test Your Changes

Before pushing, ensure:

```bash
# All tests pass
cargo test

# Code is formatted
cargo fmt

# No clippy warnings
cargo clippy -- -D warnings

# Optional: run specific tests
cargo test -p common
cargo test -p service
```

### 6. Push and Create Pull Request

```bash
# Push your branch
git push origin feature/add-conflict-resolution

# Create a pull request on GitHub
```

## Pull Request Guidelines

### PR Title

Use the same format as commit messages:

```
feat: add conflict resolution for concurrent updates
fix: handle missing pinset gracefully
docs: improve protocol specification
```

### PR Description

Include:
- **Summary** - What does this PR do?
- **Motivation** - Why is this change needed?
- **Changes** - List of key changes
- **Testing** - How did you test this?
- **Related Issues** - Link to issues (e.g., "Fixes #123")

**Example:**

```markdown
## Summary
Adds conflict resolution for concurrent updates to the same bucket.

## Motivation
Currently, concurrent updates cause sync failures. This implements automatic merge resolution using last-write-wins strategy.

## Changes
- Added `ConflictResolver` trait in `common/src/bucket/`
- Implemented LWW strategy in sync manager
- Added integration tests for concurrent writes

## Testing
- Unit tests in `test_conflict_resolution()`
- Integration test with two nodes updating simultaneously
- Manual testing with dev environment

Fixes #42
```

### PR Checklist

Before submitting, verify:

- [ ] Tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Documentation updated (if applicable)
- [ ] Conventional commit format used
- [ ] PR description is clear and complete

## Code Style

### Rust Style Guide

Follow standard Rust conventions:

```rust
// Good: snake_case for functions and variables
fn create_bucket(name: String) -> Result<Bucket> { ... }
let bucket_id = uuid::Uuid::new_v4();

// Good: PascalCase for types
struct Manifest { ... }
enum SyncStatus { ... }

// Good: SCREAMING_SNAKE_CASE for constants
const MAX_BLOB_SIZE: usize = 1024 * 1024 * 10;

// Good: Document public APIs
/// Creates a new encrypted bucket.
///
/// # Arguments
///
/// * `name` - Human-readable bucket name
///
/// # Returns
///
/// The newly created manifest
pub fn create_bucket(name: String) -> Result<Manifest> {
    // Implementation
}
```

### Error Handling

```rust
// Library crates: use custom error types
#[derive(Debug, thiserror::Error)]
pub enum BucketError {
    #[error("bucket not found: {0}")]
    NotFound(Uuid),
}

// Application code: use anyhow
use anyhow::{Context, Result};

fn load_bucket(id: Uuid) -> Result<Bucket> {
    db.get_bucket(id)
        .context("failed to load bucket")?
}
```

### Imports

Group imports logically:

```rust
// Standard library
use std::collections::HashMap;
use std::path::PathBuf;

// External crates
use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Local crates
use common::bucket::Manifest;
use common::crypto::Secret;
```

### Testing

Write tests for new functionality:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_bucket() {
        let manifest = create_bucket("test".to_string()).unwrap();
        assert_eq!(manifest.name, "test");
    }

    #[tokio::test]
    async fn test_sync_workflow() {
        // Integration test
    }
}
```

## Documentation

### Code Documentation

Document all public APIs:

```rust
/// Creates a new bucket with the given name.
///
/// The bucket is initialized with an empty root node and a randomly
/// generated encryption secret. The manifest is stored in the database
/// and the root node is written to the blob store.
///
/// # Arguments
///
/// * `name` - Human-readable name for the bucket
///
/// # Returns
///
/// The UUID of the newly created bucket
///
/// # Errors
///
/// Returns an error if:
/// - Database write fails
/// - Blob store is unavailable
///
/// # Example
///
/// ```
/// let bucket_id = create_bucket("my-files".to_string())?;
/// println!("Created bucket: {}", bucket_id);
/// ```
pub fn create_bucket(name: String) -> Result<Uuid> {
    // Implementation
}
```

### README and Guides

When updating documentation:
- Keep it concise and clear
- Include examples
- Update all affected files (README, PROTOCOL, etc.)
- Verify links work

## Review Process

### What Happens After You Submit

1. **Automated Checks**: CI runs tests, formatting, and linting
2. **Review**: Maintainers review your code
3. **Discussion**: Address feedback and questions
4. **Approval**: Once approved, PR is merged

### Responding to Feedback

- Be open to suggestions
- Ask questions if unclear
- Update your PR based on feedback
- Push additional commits (don't force-push during review)

## Community Guidelines

### Code of Conduct

- Be respectful and inclusive
- Welcome newcomers
- Focus on constructive feedback
- Assume good intentions

### Communication

- **Issues**: For bugs and feature requests
- **Discussions**: For questions and ideas
- **Pull Requests**: For code contributions

### Getting Help

Stuck? Need help? Ask!

- Open an issue with the `question` label
- Start a discussion in GitHub Discussions
- Check existing docs and issues first

## Advanced Topics

### Working with Dependencies

Update dependencies carefully:

```bash
# Update a specific dependency
cargo update -p iroh

# Check for outdated dependencies
cargo outdated

# Audit for security issues
cargo audit
```

### Performance Considerations

When optimizing:
- Profile before optimizing
- Add benchmarks for critical paths
- Document performance characteristics
- Consider memory usage and allocations

### Security Considerations

For security-related changes:
- Discuss in a private issue first (for vulnerabilities)
- Consider threat model implications
- Add tests for security properties
- Document security assumptions

## License

By contributing to JaxBucket, you agree that your contributions will be licensed under the [MIT License](LICENSE).

## Thank You!

Your contributions make JaxBucket better for everyone. Thank you for taking the time to contribute!
