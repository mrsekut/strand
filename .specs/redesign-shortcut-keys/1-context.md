# Context Analysis: Shortcut Key Redesign

## Tech Stack

- Rust TUI: ratatui 0.29 + crossterm 0.28
- Async runtime: tokio
- イベントループ: `tokio::select!` でキー入力・enrich・implement・DB pollingを多重化

## 現在のキーバインド構造

全てのキーバインドが `main.rs:56-69` の単一の `match key.code` ブロックに定義されている。

```rust
match key.code {
    KeyCode::Char('q') => break,
    KeyCode::Char('j') | KeyCode::Down => app.next(),
    KeyCode::Char('k') | KeyCode::Up => app.previous(),
    KeyCode::Enter => app.toggle_detail(),
    KeyCode::Char('e') if app.show_detail => app.edit_description(terminal).await,
    KeyCode::Char('e') => app.start_enrich(),
    KeyCode::Char('c') => app.copy_id(),
    KeyCode::Char('i') => app.start_implement(),
    KeyCode::Char('m') if app.show_detail => app.merge_impl().await,
    KeyCode::Char('d') if app.show_detail => app.discard_impl().await,
    _ => {}
}
```

### 問題点

1. **画面ごとのキー分離がない** (strand-gc3): `app.show_detail` のガード条件で分岐しているが、構造的に分離されていない。`e` が画面によって全く別のアクションになっている。
2. **j/kが詳細画面でも動く** (strand-cco): 詳細画面でテキストを読んでいるときにも `j` を押すと次のissueに移動してしまう。
3. **AI操作に接頭キーがない** (strand-0ln): `e` (enrich) と `i` (implement) がバラバラの単独キー。`a` 接頭辞で `ae` (enrich)、`ai` (implement) にまとめたい。

## 画面構造

- **一覧画面** (`show_detail == false`): issue一覧テーブル表示
- **詳細画面** (`show_detail == true`): 選択issueの全情報表示

## UIヘルプ表示

- 一覧画面タイトル: `strand - Issues (q:quit j/k:move Enter:detail c:copy e:enrich i:implement)`
- 詳細画面タイトル: `Issue Detail (Enter:back q:quit c:copy e:edit)` (impl done時は `m:merge d:discard` 追加)

## 関連ファイル

| ファイル | 役割 |
|---------|------|
| `src/main.rs` | イベントループ・キーバインド定義 |
| `src/ui.rs` | UI描画・ヘルプテキスト表示 |
| `src/app.rs` | アプリ状態・各アクションのハンドラ |

## 制約

- crossterm の `KeyCode` はシングルキーイベント。`ae` のような2キーシーケンスを処理するにはステートマシン（pending key state）が必要
- ratatui側は描画のみで入力処理に関与しない
