# Design: Shortcut Key Redesign

## Domain Models

```rust
/// キー入力の処理結果
enum KeyAction {
    Quit,
    Navigate(NavAction),
    ViewAction(ViewAction),
    None,
}

enum NavAction {
    Next,
    Previous,
    OpenDetail,  // Enter (一覧画面)
    BackToList,  // Esc (詳細画面)
}

/// 画面固有の入力モード
enum InputMode {
    Normal,
    AwaitingAI,  // `a` 押下後、次のキーを待つ状態
}
```

## Feature Boundaries

- **KeyDispatcher**: 画面状態に応じてキーイベントを適切なハンドラに振り分ける
- **ListKeyHandler**: 一覧画面のキー処理（ナビゲーション、AI操作の2キーシーケンス）
- **DetailKeyHandler**: 詳細画面のキー処理（編集、merge/discard）

依存関係: `KeyDispatcher` → `ListKeyHandler` / `DetailKeyHandler` → `App`

## 実装方針

現在の `main.rs` の `match key.code` ブロックを、画面状態で分岐する構造に変更する。新しいモジュールは作らず、`main.rs` 内の関数として分離する。

### キー処理の流れ

```
KeyEvent
  → show_detail で分岐
    → false: handle_list_key(key, app, input_mode)
    → true:  handle_detail_key(key, app)
```

### 2キーシーケンス（AI操作）

`App` に `input_mode: InputMode` フィールドを追加。

```
Normal状態 + 'a' → AwaitingAI状態に遷移
AwaitingAI状態 + 'e' → enrich実行 → Normal状態に戻る
AwaitingAI状態 + 'i' → implement実行 → Normal状態に戻る
AwaitingAI状態 + その他 → キャンセル → Normal状態に戻る
```

UI: AwaitingAI状態ではnotificationバーに `"a-..."` と表示。

### Enter/Escの分離

`toggle_detail()` を廃止し、`open_detail()` と `back_to_list()` に分離。

```rust
// App に追加
pub fn open_detail(&mut self) { self.show_detail = true; }
pub fn back_to_list(&mut self) { self.show_detail = false; }
```

## Layer Structure

```
App (状態管理・ビジネスロジック)
  ↑
main.rs キー処理関数 (入力 → App操作のマッピング)
  ↑
ui.rs (描画のみ)
```

キー処理は `main.rs` に留め、`App` には画面遷移の意図を表すメソッドのみ追加。`App` がキーコードを知る必要はない。

## 変更対象

| ファイル | 変更内容 |
|---------|---------|
| `src/app.rs` | `input_mode` フィールド追加、`open_detail()`/`back_to_list()` 追加、`toggle_detail()` 削除 |
| `src/main.rs` | キーハンドラを `handle_list_key` / `handle_detail_key` に分離 |
| `src/ui.rs` | ヘルプテキスト更新、AwaitingAI状態の表示 |
