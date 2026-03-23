#!/usr/bin/env bash
set -euo pipefail

SANDBOX="/tmp/strand-sandbox"

if [ -d "$SANDBOX" ]; then
  echo "Removing existing sandbox..."
  rm -rf "$SANDBOX"
fi

echo "Creating sandbox at $SANDBOX..."
mkdir -p "$SANDBOX"
cd "$SANDBOX"

git init -q
git commit --allow-empty -m "init" -q
bd init --skip-hooks --skip-merge-driver -q

# === Issues ===
# bd q でIDを変数に取って依存関係を組む

# --- P0: 緊急 ---
LOGIN_BUG=$(bd q "ログインボタンが押せない" -t bug -p 0 -l "bug,frontend,ios")
AUTH_TOKEN=$(bd q "認証トークンの有効期限管理" -t bug -p 0 -l "bug,security,backend")

# --- P1: 高優先 ---
SEARCH_SLOW=$(bd q "検索APIのレスポンスが遅い" -t bug -p 1 -l "bug,backend,performance")
CICD=$(bd q "CI/CDパイプラインのセットアップ" -t task -p 1 -l "infra,ci")
SECURITY_AUDIT=$(bd q "セキュリティ監査の実施" -t task -p 1 -l "security,task")

# --- P2: 中優先 ---
DARKMODE=$(bd q "ダークモード対応" -t feature -p 2 -l "feature,frontend,ux")
DB_MIGRATE=$(bd q "DBマイグレーションの自動化" -t task -p 2 -l "infra,backend")
NOTIFY_DESIGN=$(bd q "通知システムの設計" -t feature -p 2 -l "feature,backend,design")
RATE_LIMIT=$(bd q "APIレートリミットの実装" -t task -p 2 -l "backend,api")
RESPONSIVE=$(bd q "レスポンシブデザインの改善" -t feature -p 2 -l "feature,frontend,mobile")
LOG_INFRA=$(bd q "ログ収集基盤の構築" -t task -p 2 -l "infra,observability")
WEBHOOK=$(bd q "Webhook配信の信頼性向上" -t task -p 2 -l "backend,api")
CACHE=$(bd q "キャッシュ戦略の見直し" -t task -p 2 -l "backend,performance")
FILE_UPLOAD=$(bd q "ファイルアップロード機能" -t feature -p 2 -l "feature,backend,frontend")
ERROR_HANDLING=$(bd q "エラーハンドリングの統一" -t task -p 2 -l "backend,api,dx")
MONITORING=$(bd q "監視ダッシュボードの構築" -t task -p 2 -l "infra,observability")
PAGINATION=$(bd q "ページネーションの改善" -t bug -p 2 -l "bug,backend,api")

# --- P3: 低優先 ---
PROFILE=$(bd q "ユーザープロフィール編集画面" -t feature -p 3 -l "feature,frontend")
E2E_TEST=$(bd q "E2Eテストの導入" -t task -p 3 -l "test,infra")
A11Y=$(bd q "アクセシビリティ対応" -t feature -p 3 -l "feature,frontend,a11y")
BATCH=$(bd q "バッチ処理フレームワーク" -t feature -p 3 -l "feature,backend,infra")
GRAPHQL=$(bd q "GraphQL APIの導入検討" -t task -p 3 -l "backend,api,design")
I18N=$(bd q "多言語対応(i18n)" -t feature -p 3 -l "feature,frontend,i18n")

# --- P4: バックログ ---
ISSUE_TEMPLATE=$(bd q "issueテンプレートの整備" -t chore -p 4 -l "chore,dx")
CODEOWNERS=$(bd q "コードオーナー設定" -t chore -p 4 -l "chore,dx")

# === 依存関係 ===
# 構造: A depends on B = AをやるにはBが先に必要

# CI/CD系チェーン: E2Eテスト → CI/CD → DBマイグレーション
bd dep add "$E2E_TEST" "$CICD"
bd dep add "$CICD" "$DB_MIGRATE"

# 通知チェーン: 通知システム → Webhook → エラーハンドリング統一
bd dep add "$NOTIFY_DESIGN" "$WEBHOOK"
bd dep add "$WEBHOOK" "$ERROR_HANDLING"

# 監視系チェーン: 監視ダッシュボード → ログ収集基盤
bd dep add "$MONITORING" "$LOG_INFRA"

# セキュリティ系: セキュリティ監査 → 認証トークン修正
bd dep add "$SECURITY_AUDIT" "$AUTH_TOKEN"

# 検索の高速化 → キャッシュ + ページネーション両方必要
bd dep add "$SEARCH_SLOW" "$CACHE"
bd dep add "$SEARCH_SLOW" "$PAGINATION"

# ファイルアップロード → プロフィール編集（アバター画像に使う）
bd dep add "$PROFILE" "$FILE_UPLOAD"

# i18n → ダークモード（テーマ設定と一緒にやる）
bd dep add "$I18N" "$DARKMODE"

# GraphQL → エラーハンドリング統一 + ページネーション改善
bd dep add "$GRAPHQL" "$ERROR_HANDLING"
bd dep add "$GRAPHQL" "$PAGINATION"

# バッチ処理 → ログ収集基盤（ジョブの実行ログを残す）
bd dep add "$BATCH" "$LOG_INFRA"

# レスポンシブ → ダークモード（テーマ周りを先にやる）
bd dep add "$RESPONSIVE" "$DARKMODE"

# APIレートリミット → エラーハンドリング統一
bd dep add "$RATE_LIMIT" "$ERROR_HANDLING"

# === Closed issues（完了済み） ===
LINT=$(bd q "ESLint + Prettierの導入" -t task -p 2 -l "dx,frontend")
bd close "$LINT" -r "ESLint flat config + Prettierで設定完了"

REPO_INIT=$(bd q "リポジトリ初期構成" -t task -p 1 -l "infra")
bd close "$REPO_INIT" -r "monorepo構成、tsconfig、package.json整備完了"

DESIGN_SYSTEM=$(bd q "デザインシステムの基盤" -t task -p 2 -l "frontend,design")
bd close "$DESIGN_SYSTEM" -r "Tailwind + shadcn/uiで基盤構築済み"

API_SKELETON=$(bd q "REST APIのスケルトン実装" -t task -p 1 -l "backend,api")
bd close "$API_SKELETON" -r "Express + zodでルーティング・バリデーション基盤完了"

AUTH_BASIC=$(bd q "基本認証フローの実装" -t feature -p 0 -l "feature,backend,security")
bd close "$AUTH_BASIC" -r "JWT + refresh tokenフロー実装済み"

echo ""
echo "=== Sandbox ready ==="
echo "Open: $(bd count 2>/dev/null || echo '?')"
echo "Closed: $(bd count --all 2>/dev/null || echo '?') (includes closed)"
echo "Dependencies: 13"
echo ""
echo "Usage: cargo run -- --dir $SANDBOX"
