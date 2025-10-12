# Release Process

This document describes how to release new versions of the JaxBucket crates using `cargo-smart-release`.

## Overview

We use [cargo-smart-release](https://crates.io/crates/cargo-smart-release) to:
- Automatically bump crate versions based on conventional commits
- Generate and update CHANGELOGs
- Create git tags per crate
- Keep workspace dependencies in sync

Publishing to crates.io is handled automatically by GitHub Actions when tags are pushed.

## Prerequisites

1. **Install cargo-smart-release** (if not already installed):
   ```bash
   cargo install cargo-smart-release
   ```

2. **Use conventional commits** for your changes:
   - `feat:` - New feature (minor version bump, e.g., 0.1.0 → 0.2.0)
   - `fix:` - Bug fix (patch version bump, e.g., 0.1.0 → 0.1.1)
   - `feat!:` or `BREAKING CHANGE:` - Breaking change (major version bump, e.g., 0.1.0 → 1.0.0)
   - `docs:`, `chore:`, `refactor:`, etc. - No version bump, but included in changelog

## When to Release

### From the Command Line (Current Workflow)

This is the recommended approach for now:

1. **Make sure all changes are committed and pushed**
   ```bash
   git status  # Should be clean
   ```

2. **Preview what would happen** (dry-run):
   ```bash
   cargo smart-release jax-bucket -v
   ```

   This shows:
   - Which versions will be bumped
   - What changelog entries will be generated
   - What git tags will be created

3. **Execute the release**:
   ```bash
   cargo smart-release jax-bucket --execute --no-publish
   ```

   This will:
   - Update version numbers in all Cargo.toml files
   - Update CHANGELOGs based on commits since last release
   - Create git tags (e.g., `jax-common-v0.1.1`, `jax-service-v0.1.1`, `jax-bucket-v0.1.1`)
   - Commit the changes with message like "release"
   - Push tags and commits to GitHub
   - **Note**: It does NOT publish to crates.io (that's handled by GitHub Actions)

4. **GitHub Actions will automatically**:
   - Trigger on the new tags
   - Run tests
   - Publish updated crates to crates.io

## Flags Explained

- `--execute` - Actually perform the release (without this, it's a dry-run)
- `--no-publish` - Don't run `cargo publish` (we let GitHub Actions do that)
- `-v` - Verbose output showing what would happen
- `--allow-dirty` - Allow releasing with uncommitted changes (not recommended)
- `--update-crates-index` - Update local crates.io index first (good for accuracy)

## Releasing Individual Crates

If you only want to bump a specific crate (not the whole workspace):

```bash
# Release only jax-common
cargo smart-release jax-common --execute --no-publish

# Release only jax-service (will also bump jax-common if it depends on unreleased changes)
cargo smart-release jax-service --execute --no-publish
```

The tool will automatically determine which dependencies need to be updated.

## Manual Version Bumps

If you want to force a specific version bump instead of auto-detection:

```bash
# Force a minor version bump
cargo smart-release jax-bucket --execute --no-publish --bump minor

# Force a patch version bump
cargo smart-release jax-bucket --execute --no-publish --bump patch

# Force a major version bump
cargo smart-release jax-bucket --execute --no-publish --bump major
```

## Editing Changelogs Manually

If the auto-generated changelog is empty or needs tweaking:

1. Run the release command (it will stop if changelog is empty)
2. Manually edit the CHANGELOG.md files in each crate directory
3. Re-run the release command

Or use:
```bash
cargo changelog --write jax-bucket
```

## Automating Releases via PR (Future)

Currently, releases are done manually from the command line. In the future, we could automate this with:

### Option 1: Release PR Bot
- Use a bot that creates "Release PR" when commits accumulate
- PR shows the changelog preview
- Merging the PR triggers the release

### Option 2: GitHub Actions Workflow Dispatch
- Add a manual workflow that runs `cargo-smart-release`
- Triggered via GitHub UI "Run workflow" button
- Would need to configure git credentials in the action

### Option 3: Label-Based Releases
- Add a label like `release:ready` to a PR
- On merge, a GitHub Action runs the release process

**For now, manual command-line releases are simpler and give you full control.**

## Troubleshooting

### "Changelog is empty"
Add more descriptive commit messages using conventional commits format, or manually edit the CHANGELOG.md files.

### "Working tree has changes"
Commit or stash your changes first. Use `--allow-dirty` only if absolutely necessary.

### "Crate already published"
The tool detects if a version is already on crates.io. You may need to bump to a higher version.

### Tags not pushing
Make sure you have push permissions to the repository. Check `git remote -v` to confirm the remote URL.

## Files

- `release.toml` - Configuration for cargo-smart-release
- `crates/*/CHANGELOG.md` - Per-crate changelogs
- `.github/workflows/publish-crate.yml` - Automated publishing workflow

## Examples

### Typical Release Flow

```bash
# 1. Check what's changed
git log --oneline

# 2. Dry-run to preview
cargo smart-release jax-bucket -v

# 3. Execute if everything looks good
cargo smart-release jax-bucket --execute --no-publish

# 4. Wait for GitHub Actions to publish to crates.io
# Check: https://github.com/jax-ethdenver-2025/jax-bucket/actions
```

### Emergency Hotfix

```bash
# Make your fix with a conventional commit
git commit -m "fix: critical security issue in authentication"

# Release immediately
cargo smart-release jax-bucket --execute --no-publish --bump patch
```
