# strand

beads CLIのラッパーTUI。大量のissueを雑に投げ込み、AIが自動で精緻化・プロトタイピングし、人間がTUI上でtriage（承認/却下/統合）する。

## 前提

- Rust (cargo)
- [beads](https://github.com/anthropics/beads) CLI (`bd` コマンド)

## ビルド

```bash
cargo build
```

## 開発用サンドボックス

本リポジトリのbeadsとテストデータが混ざらないよう、`/tmp`に別リポジトリを作ってテストする。

### セットアップ

```bash
bash scripts/setup-sandbox.sh
```

`/tmp/strand-sandbox/` に以下が作られる:

- issue 30件（P0〜P4、bug/feature/task/chore混在）
- closed 5件（完了理由付き）
- 依存関係 15本（チェーン、扇形、合流を含む）

再実行すると既存のサンドボックスを削除して作り直す。

### TUI起動

```bash
cargo run -- --dir /tmp/strand-sandbox
```

### キー操作

| キー | 操作 |
|------|------|
| `j` / `↓` | 次のissue |
| `k` / `↑` | 前のissue |
| `Enter` | 詳細表示 / 戻る |
| `q` | 終了 |
