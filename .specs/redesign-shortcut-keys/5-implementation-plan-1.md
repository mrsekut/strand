# Implementation Plan: Shortcut Key Redesign

単一PRで実装する。変更は3ファイルに閉じており、分割の必要はない。

## Tasks

### 1. App に InputMode と画面遷移メソッド追加 (`src/app.rs`)

- [ ] `InputMode` enum を定義 (`Normal`, `AwaitingAI`)
- [ ] `App` に `pub input_mode: InputMode` フィールド追加
- [ ] `open_detail()` メソッド追加 (`self.show_detail = true`)
- [ ] `back_to_list()` メソッド追加 (`self.show_detail = false`)
- [ ] `toggle_detail()` を削除

### 2. キーハンドラを画面別に分離 (`src/main.rs`)

- [ ] `handle_list_key()` 関数を作成
  - `↑`/`↓`: `app.next()` / `app.previous()`
  - `Enter`: `app.open_detail()`
  - `c`: `app.copy_id()`
  - `a`: `app.input_mode = AwaitingAI` + notification表示
  - AwaitingAI状態: `e` → enrich / `i` → implement / 他 → キャンセル
- [ ] `handle_detail_key()` 関数を作成
  - `Esc`: `app.back_to_list()`
  - `↑`/`↓`: `app.next()` / `app.previous()`
  - `c`: `app.copy_id()`
  - `e`: `app.edit_description(terminal).await`
  - `m`: `app.merge_impl().await` (impl done時)
  - `d`: `app.discard_impl().await` (impl done時)
- [ ] 既存の `match key.code` ブロックを `show_detail` 分岐 + 上記関数呼び出しに置換
- [ ] `q` は分岐前の共通処理として残す

### 3. UIヘルプテキスト更新 (`src/ui.rs`)

- [ ] 一覧画面タイトル: `q:quit ↑↓:move Enter:detail c:copy a→e:enrich a→i:implement`
- [ ] 詳細画面タイトル: `Esc:back q:quit ↑↓:move c:copy e:edit` (impl done時: `m:merge d:discard` 追加)
- [ ] AwaitingAI状態の通知表示（notificationバーに `"a-..."` を出す）

### 4. 検証

- [ ] `cargo build` 通過
- [ ] `cargo clippy` 警告なし

## Commit Suggestion

```
feat: redesign shortcut keys - separate views, arrow-only nav, AI prefix key

- Separate key handlers for list/detail views (strand-gc3)
- Remove j/k navigation, use arrow keys only (strand-cco)
- Add 'a' prefix for AI operations: ae=enrich, ai=implement (strand-0ln)
- Use Esc to go back from detail, Enter only opens detail (strand-nuq)
```
