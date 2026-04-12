#!/usr/bin/env bash
# Stop hook: cargo build の warning を検出して Claude に修正させる。
# warning があれば exit 2 で stop をブロックし、stderr の内容を Claude に返す。

set -u

cd "$(dirname "$0")/../.." || exit 0

OUTPUT=$(cargo build --message-format=short 2>&1)
STATUS=$?

if [ $STATUS -ne 0 ]; then
  # ビルド失敗自体は別途扱う。stop はブロックしない。
  exit 0
fi

WARNINGS=$(printf '%s\n' "$OUTPUT" | grep -E '^(warning|.+: warning)' || true)

if [ -n "$WARNINGS" ]; then
  {
    echo "cargo build に warning があります。すべて修正してから停止してください:"
    echo
    printf '%s\n' "$WARNINGS"
  } >&2
  exit 2
fi

exit 0
