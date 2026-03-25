# strand

A TUI wrapper for the beads CLI. Throw in a bunch of issues, let AI automatically refine and prototype them, and triage (approve/reject/merge) on the TUI.

## Prerequisites

- Rust (cargo)
- [beads](https://github.com/anthropics/beads) CLI (`bd` command)

## Build

```bash
cargo build
```

## Development Sandbox

To keep beads data and test data separate from this repository, create a test repository in `/tmp`.

### Setup

```bash
bash scripts/setup-sandbox.sh
```

This creates the following in `/tmp/strand-sandbox/`:

- 30 issues (P0–P4, mix of bug/feature/task/chore)
- 5 closed issues (with completion reasons)
- 15 dependencies (chains, fan-out, and convergence patterns)

Re-running the script deletes and recreates the existing sandbox.

### Launch TUI

```bash
cargo run -- --dir /tmp/strand-sandbox
```

### Key Bindings

| Key       | Action             |
| --------- | ------------------ |
| `j` / `↓` | Next issue         |
| `k` / `↑` | Previous issue     |
| `Enter`   | Toggle detail view |
| `q`       | Quit               |
