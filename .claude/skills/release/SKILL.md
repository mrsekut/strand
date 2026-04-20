---
name: release
description: "Release a new version. Bumps version in Cargo.toml and flake.nix, commits, tags, publishes to crates.io, and pushes. Usage: /release 0.2.2"
---

# Release

Automate the full release flow for this Rust + Nix project.

## Usage

```
/release <version>
```

Example: `/release 0.2.2`

## Workflow

The argument is the new version string (e.g. `0.2.2`). If no version is provided, ask the user.

Execute these steps sequentially. Stop and report if any step fails.

### 1. Validate

- Confirm the working tree is clean (`git status --porcelain` should be empty, ignoring `.beads/`).
- Confirm the version argument looks like semver (e.g. `X.Y.Z`).

### 2. Bump versions

Update the version string in both files:

- `Cargo.toml`: the `version = "..."` line under `[package]`
- `flake.nix`: the `version = "...";` line

Use the Edit tool for both (parallel).

### 3. Update lockfile

```bash
cargo build
```

This regenerates `Cargo.lock` to match the new version.

### 4. Commit and tag

```bash
git add Cargo.toml Cargo.lock flake.nix
git commit -m "chore: bump version to <version>"
git tag v<version>
```

### 5. Publish

```bash
cargo publish
```

### 6. Push

```bash
git push && git push --tags
```

### 7. Report

Tell the user the release is complete with the version and crates.io package name.
