# Development Guide

## Build & Test

```bash
cargo build
cargo test
cargo run -- --dir /tmp/strand-sandbox  # sandbox for testing
```

## Ubiquitous Language

| Term             | Definition                                                              | Code                                       |
| ---------------- | ----------------------------------------------------------------------- | ------------------------------------------ |
| Issue            | Work unit managed by strand. Persisted in beads DB                      | `bd::Issue`                                |
| Epic             | Issue with children. `issue_type == "epic"`. Created by Split           | `Issue` (issue_type="epic")                |
| Child            | Issue under an Epic. Created via `bd create --parent`                   | `Issue` (with parent)                      |
| Enrich           | AI analyzes issue, appends problems + solution proposals to description | `ai_enrich::run()`                         |
| Split            | AI decomposes issue into subtasks, creating Epic + Children structure   | `ai_split::run()`                          |
| Implement (Impl) | Creates git worktree, AI writes code                                    | `ai_implement::run()`                      |
| ImplJob          | Tracks Running/Done/Failed state of an Impl                             | `ai_implement::ImplJob`                    |
| Merge            | Integrates Impl result into target branch (master or epic branch)       | `ai_implement::merge::merge_into_branch()` |
| Discard          | Discards Impl result. Removes worktree and branch                       | `ImplManager::discard()`                   |
| Worktree         | Isolated working directory for Impl                                     | `../strand-impl-{short_id}`                |
| EpicBranch       | Intermediate branch where child Impls merge into                        | `epic/{epic_id}`                           |
| ImplBranch       | Branch for individual Impl work                                         | `impl/{issue_id}`                          |

### Labels (State Markers)

| Label                 | Meaning              | Added             | Removed        |
| --------------------- | -------------------- | ----------------- | -------------- |
| `strand-needs-enrich` | Not yet enriched     | Issue creation    | Enrich start   |
| `strand-enriched`     | Enrich completed     | Enrich completion | —              |
| `strand-unread`       | Unread enrich result | Enrich completion | Opening detail |

## Architecture

### Naming Convention

- `page_*` — TUI view modules (keys.rs + ui.rs)
- `ai_*` — AI workflow modules (manager.rs + prompt.rs + run.rs)

### Design Patterns

- **Manager pattern**: Each AI workflow has a Manager struct (`EnrichManager`, `ImplManager`, `SplitManager`) that owns its state and is independent of App. App delegates to managers.
- **View enum with state**: `View` enum variants hold view-specific state (scroll_offset, children, diff, etc.). State is moved between variants on navigation via `std::mem::replace`.
- **Event channels**: Each AI workflow communicates via `mpsc` channels. Managers spawn tokio tasks and send events back to App.

## beads CLI Integration

strand calls `bd` CLI as subprocess. No custom data store — all issue data lives in beads.

| strand operation | beads CLI                                    |
| ---------------- | -------------------------------------------- |
| Add issue        | `bd q` / `bd create`                         |
| Show detail      | `bd show <id> --json`                        |
| List issues      | `bd list --json`                             |
| Edit             | `bd update --title/--description/--priority` |
| Close            | `bd close <id>`                              |
| Add dependency   | `bd dep add`                                 |
| Label            | `bd label add/remove`                        |
| Create child     | `bd create --parent <epic_id>`               |

## Tech Stack

- TUI: ratatui + crossterm
- Async: tokio
- AI: Claude Code CLI (`claude -p`)
- Issue tracking: beads CLI (`bd`)
- Serialization: serde + serde_json
