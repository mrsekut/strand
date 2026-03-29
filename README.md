# strand

AI-assisted Issue Triage Cockpit — a TUI wrapper for the beads CLI.

Throw in a bunch of issues, let AI automatically refine and prototype them, and triage (approve/reject/merge) on the TUI.

## Concept

**"Throw it in rough, AI will structure and organize it for you."**

Three loops drive the workflow:

1. **Capture** — `strand q "button is broken"` for zero-friction issue creation
2. **Enrich** — AI analyzes problems and generates solution proposals in the background
3. **Triage** — Review AI proposals on the TUI and approve/reject/defer at speed

## Prerequisites

- Rust (cargo)
- [beads](https://github.com/anthropics/beads) CLI (`bd` command)
- [Claude Code](https://claude.ai/claude-code) CLI (`claude` command)

## Build

```bash
cargo build
```

## Usage

### Launch TUI

```bash
strand              # current directory
strand --dir /path  # specify directory
```

### Quick Capture

```bash
strand q "issue title"
```

Creates a P2 task with auto-enrich enabled.

## Key Bindings

### Issue List

| Key       | Action             |
| --------- | ------------------ |
| `j` / `↓` | Next issue         |
| `k` / `↑` | Previous issue     |
| `Enter`   | Open detail        |
| `c`       | Copy issue ID      |
| `p`       | Set priority (0-4) |
| `a`       | AI menu            |
| `x`       | Close issue        |
| `q`       | Quit               |

### AI Menu (after `a`)

| Key | Action         |
| --- | -------------- |
| `e` | Enrich         |
| `i` | Implement      |
| `s` | Split to tasks |

### Issue Detail / Child Detail

| Key   | Action              |
| ----- | ------------------- |
| `Esc` | Back                |
| `j/k` | Scroll              |
| `c`   | Copy issue ID       |
| `p`   | Copy worktree path  |
| `e`   | Edit (open $EDITOR) |
| `a`   | AI menu             |
| `m`   | Merge impl          |
| `d`   | Discard impl        |
| `x`   | Close issue         |

### Epic Detail

| Key     | Action                                          |
| ------- | ----------------------------------------------- |
| `Esc`   | Back                                            |
| `j/k`   | Select child                                    |
| `Enter` | Open child detail                               |
| `c`     | Copy issue ID                                   |
| `e`     | Edit epic                                       |
| `a`     | AI menu                                         |
| `m`     | Merge epic to master (when all children closed) |
| `x`     | Close                                           |

## Status Icons

| Icon | Meaning        |
| ---- | -------------- |
| `⚡` | Impl running   |
| `✓`  | Done / Closed  |
| `✗`  | Failed         |
| `⟳`  | Enriching      |
| `●`  | Unread         |
| `○`  | Ready to merge |
| `·`  | Pending        |

## Development Sandbox

```bash
bash scripts/setup-sandbox.sh
cargo run -- --dir /tmp/strand-sandbox
```

## Git Branch Strategy

```
master ──────────────────────────────────────→
  │                                ↑
  └── epic/{epic_id} ───────────→ merge (m key)
        │          ↑      ↑
        ├── impl/{child_1} → merge (m key)
        └── impl/{child_2} → merge (m key)

master ──────────────────────────────────────→
  │                    ↑
  └── impl/{issue_id} → merge (m key)   ← standalone
```
