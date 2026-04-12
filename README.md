# strand

**Throw issues in, AI writes the code.**

An AI-powered TUI frontend for [beads](https://github.com/steveyegge/beads). Throw in rough issue descriptions — AI analyzes, splits into subtasks, implements each one in isolated git worktrees, and you just review & merge.

<p align="center">
  <img src="assets/demo.gif" alt="strand demo" width="800" />
</p>

## How it works

1. **Capture** — `strand q "button is broken"` to create an issue in one shot
2. **Enrich** — AI analyzes the problem and proposes solutions in the background
3. **Split** — AI decomposes the issue into concrete subtasks
4. **Implement** — AI writes code in isolated git worktrees, one per task
5. **Merge** — Review the diff on the TUI and merge with a single keystroke

## Install

```bash
cargo install strand-tui
# or
nix profile install github:mrsekut/strand
```

### Prerequisites

- [beads](https://github.com/steveyegge/beads) CLI (`bd` command) — issue tracking backend
- [Claude Code](https://claude.ai/claude-code) CLI (`claude` command) — AI engine

## Quick Start

```bash
# Create an issue
strand q "fix the login bug"

# Open the TUI
strand
```

From the TUI, press `a` to open the AI menu — enrich, split, or implement any issue.
