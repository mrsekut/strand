# bdtui: AI-assisted Issue Triage Cockpit

beads CLIのラッパーTUI。大量のissueを雑に投げ込み、AIが自動で精緻化・プロトタイピングし、人間がTUI上でtriage（承認/却下/統合）する。

## 背景・課題

- 100+個のissueを管理したいが、人間が全部を精緻化・優先順位付けするのは認知コストが高い
- 「管理できないなら追加しても意味がない」という心理的障壁で、issueの投入自体が滞る
- issueはフラットなTODOリストではない:
  - AをやればB,C,Dが不要になることがある
  - A,B,Cは一緒にやるべきことがある
  - 全部やる必要はなく、課題の大きさやソリューションの筋の良さを見て判断が必要

## コアコンセプト

**「雑に投げ込んでも、AIが構造化・整理してくれる」という安心感**

これにより入力の心理的障壁が消え、issueの投入 → AIによる精緻化 → 人間によるtriageのループが回る。

## 3つのループ

### Loop 1: Capture（摩擦ゼロで投げ込む）

- beads CLIの `bd q` で雑にissue追加（titleだけでOK）
- TUIからもワンキーで追加できる
- この段階では「ボタンがおかしい」程度の粒度で良い

### Loop 2: Enrich（AIが勝手に育てる）

バックグラウンドで自動実行。人間は待たない。

- **description補完**: 雑なtitleから課題の背景・影響範囲を推定して記述
- **ソリューション案生成**: 解決策を複数案（例: 3つ）生成し、各案のpros/consを記述
- **類似issue検出**: vector searchでセマンティックな類似度を計算し、関連issueを紐付け
- **依存関係の推定**: 既存issueとの依存関係（blocks/blocked_by）を推定・提案
- **プロトタイプ実装**: 別branchでソリューション案を実際にコード実装（Claude Codeを並行spawn）
  - 5つ並行で走らせ、最初の1-2個が採用されたら残りは破棄してよい

### Loop 3: Triage（人間がTUI上で判断する）

AIが育てたissueを見渡して、高速に意思決定する。

- **プロト採用**: merge, 関連issueをclose
- **却下**: close, 理由を記録
- **保留**: priority調整, defer
- **統合**: 「A,B,Cは実はこの1issueに集約すべき」→ bd duplicate / bd supersede

## TUIの画面構成

### 1. Inbox

AIがenrichした結果の「レビュー待ち」キュー。

- enrich完了したissueが新着順に並ぶ
- 各issueに対してAIが生成したもの（spec、ソリューション案、プロト有無）がサマリ表示される
- ここから個別のReview画面に遷移

### 2. Dashboard

全issue俯瞰。

- クラスタ表示（類似issueをグルーピング）
- 依存関係グラフ
- priority分布
- status別カウント
- 「今やるべきissue」のAI推薦（依存関係 + priority + 影響範囲を考慮）

### 3. Review

1つのissueを詳細に見てAIの提案を承認/却下する画面。

- issue詳細（title, description, labels, priority, deps）
- AIが生成したソリューション案一覧
- プロトタイプのdiff表示
- 承認/却下/修正のアクション
- 関連issueへのナビゲーション

### 4. Batch Triage

類似issueをまとめて高速に捌く画面。

- 類似度が高いissueのグループが一覧表示
- 「統合」「片方を閉じる」「無関係」を素早く判定
- 一括操作（まとめてclose、まとめてlabel付与等）

## beads CLIとの対応

bdtuiは内部的にbeads CLIを呼び出す。独自のデータストアは持たない（または最小限）。

| bdtui操作      | beads CLI                                                              |
| -------------- | ---------------------------------------------------------------------- |
| issue追加      | `bd q` / `bd create`                                                   |
| 詳細表示       | `bd show <id> --json`                                                  |
| 一覧取得       | `bd list --json`                                                       |
| 編集           | `bd update --title/--description/--priority`                           |
| クローズ       | `bd close <id>`                                                        |
| 依存関係追加   | `bd dep add`                                                           |
| 依存グラフ     | `bd graph <id>` / `bd dep tree --format=mermaid`                       |
| 重複マーク     | `bd duplicate <id> --of <canonical>`                                   |
| 上位互換で置換 | `bd supersede <id> --with <new>`                                       |
| ラベル操作     | `bd label add/remove`                                                  |
| 類似検出       | `bd duplicates`（ハッシュ一致のみ、セマンティック類似はbdtui側で実装） |
| stale検出      | `bd stale`                                                             |
| blocked一覧    | `bd blocked`                                                           |
| 検索           | `bd search`                                                            |

## 技術スタック

```
bdtui (Rust)
├── TUI: ratatui + crossterm
├── 非同期: tokio
├── beads連携: bd CLIをsubprocessとして呼び出し（--no-daemon --json）
├── AI連携
│   ├── Claude Code: subprocessとしてspawn（spec生成、プロト実装）
│   └── Embedding/Vector Search: 類似度計算（ローカル or Claude API）
└── データ
    ├── issueデータ: beadsが管理（bdtuiは持たない）
    └── AI生成メタデータ: ローカルに保持（enrich結果、ソリューション案、プロトbranch情報等）
```

### Rustを採用する理由

- ratatuiは複雑なTUI（パネル分割、スクロール、依存グラフ可視化）に最も実績がある
- tokioでバックグラウンドタスク（Claude Code複数spawn、bd CLI呼び出し）とUI描画を両立
- シングルバイナリ配布が可能

## 段階的な開発計画

### Phase 0: 最小TUI

- `bd list --json` を表示、j/k/Enterでナビゲーション
- issue詳細表示
- 基本操作（close, priority変更, label追加）

### Phase 1: Enrich基盤

- issueに対してClaude Codeを呼び出してdescription補完・ソリューション案生成
- 結果をbeadsのissueフィールド（description, design, notes等）に書き戻す
- バックグラウンド実行、TUI上で完了通知

### Phase 2: 類似度・クラスタリング

- issue titleとdescriptionのembedding計算
- 類似issueの検出と表示
- Batch Triage画面

### Phase 3: プロトタイプ自動生成

- ソリューション案に基づき、別branchでClaude Codeにプロト実装させる
- 並行実行管理
- Review画面でdiff表示、採用/却下

### Phase 4: ダッシュボード・推薦

- 全体俯瞰ダッシュボード
- 「今やるべきissue」のAI推薦
- 依存関係グラフの対話的表示
