#!/usr/bin/env bash
set -eo pipefail
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

payload="${PAYLOAD:-$RUNNER_TEMP/echoes-payload}"
case "$payload" in
  /*) ;;
  *) payload="$PWD/$payload" ;;
esac
if [ -e "$payload/meta.json" ]; then
  exec bash "$ECHOES_LIB/review.sh"
fi
reject() {
  echo "echoes: $(escape_data "$1")"
  echo rejected > "$RUNNER_TEMP/echoes-post-status"
  exit 1
}
[ -d "$payload" ] && [ ! -L "$payload" ] || reject "no prepared payloads in $payload"
payloads=()
while IFS= read -r -d '' child; do
  payloads+=("$child")
  [ "${#payloads[@]}" -le 64 ] || reject "more than 64 prepared payloads in $payload"
done < <(find -P "$payload" -mindepth 1 -maxdepth 1 -type d -print0)
[ "${#payloads[@]}" -gt 0 ] || reject "no prepared payloads in $payload"

work="$(mktemp -d "$RUNNER_TEMP/echoes-post.XXXXXX")"
trap 'rm -rf -- "$work"' EXIT
failed=0
index=0
for child in "${payloads[@]}"; do
  index=$((index + 1))
  mkdir "$work/$index"
  echo "echoes: reviewing prepared payload $(escape_data "$child")"
  if RUNNER_TEMP="$work/$index" PAYLOAD="$child" bash "$ECHOES_LIB/review.sh"; then
    echo "echoes: processed prepared payload $(escape_data "$child")"
  else
    echo "echoes: failed prepared payload $(escape_data "$child")"
    failed=1
  fi
done
if [ "$failed" = 1 ]; then
  echo failed > "$RUNNER_TEMP/echoes-post-status"
else
  echo processed > "$RUNNER_TEMP/echoes-post-status"
fi
exit "$failed"
