#!/usr/bin/env bash
set -eo pipefail
# The base report lets the review ignore duplicates that already exist on the
# base commit. Any failure degrades to reviewing the full report.
base_sha="$(gh api "repos/$GITHUB_REPOSITORY/pulls/$PR_NUMBER" --jq '.base.sha' 2>/dev/null)" || base_sha=''
if [ -z "$base_sha" ]; then
  echo "echoes: cannot read the base commit, the full report is reviewed"
  exit 0
fi
if ! git -C "$GITHUB_WORKSPACE" fetch -q --depth 1 origin "$base_sha" \
  || ! git -C "$GITHUB_WORKSPACE" worktree add -q --detach "$RUNNER_TEMP/echoes-base" "$base_sha"; then
  echo "echoes: cannot check out the base commit, the full report is reviewed"
  exit 0
fi
printf '%s\n' "$base_sha" > "$RUNNER_TEMP/echoes-base-sha"
dir="$RUNNER_TEMP/echoes-base"
[ -n "$CWD" ] && dir="$dir/$CWD"
set --
[ "$ALL_TARGETS" = "true" ] && set -- --all-targets "$@"
[ -n "$PACKAGE" ] && set -- --package "$PACKAGE" "$@"
[ "$FUNCTIONS" = "true" ] && set -- --functions "$@"
[ -n "$FN_MIN_TOKENS" ] && set -- --fn-min-tokens "$FN_MIN_TOKENS" "$@"
[ "$CONTEXT_BLOCKS" = "true" ] && set -- --context-blocks "$@"
[ -n "$CONTEXT_MIN_TOKENS" ] && set -- --context-min-tokens "$CONTEXT_MIN_TOKENS" "$@"
[ -n "$CONTEXT_SELECT" ] && set -- --context-select "$CONTEXT_SELECT" "$@"
[ -n "$CONTEXT_SEARCH" ] && set -- --context-search "$CONTEXT_SEARCH" "$@"
if [ "$CONTEXT_DEPTH" != "0" ]; then
  set -- --context-dependency-depth "$CONTEXT_DEPTH" "$@"
  [ "$CONTEXT_DEV" = "true" ] && set -- --context-dev-dependencies "$@"
  [ "$CONTEXT_BUILD" = "true" ] && set -- --context-build-dependencies "$@"
  [ "$CONTEXT_ALL_OPT" = "true" ] && set -- --context-all-optional-dependencies "$@"
  [ -n "$CONTEXT_FEATURES" ] && set -- --context-features "$CONTEXT_FEATURES" "$@"
  [ "$CONTEXT_ALL_FEATURES" = "true" ] && set -- --context-all-features "$@"
  [ "$CONTEXT_NO_DEFAULT" = "true" ] && set -- --context-no-default-features "$@"
  [ -n "$CONTEXT_TARGET" ] && set -- --context-target "$CONTEXT_TARGET" "$@"
  [ -n "$CONTEXT_CACHE_DIR" ] && set -- --context-cache-dir "$CONTEXT_CACHE_DIR" "$@"
  [ "$CONTEXT_CHECK_LATEST" = "false" ] && set -- --context-check-latest false "$@"
  [ "$CONTEXT_FAIL" = "true" ] && set -- --context-fail-on-reuse "$@"
fi
while read -r pattern; do
  [ -n "$pattern" ] && set -- "$@" --exclude "$pattern"
done <<< "$EXCLUDE"
# Threshold 1 inventories every base site, a group the pull request
# completes from a single existing copy must still be reported.
# The base tree has the head's layout, so the head's prefix roots its paths.
base_report="$RUNNER_TEMP/echoes-base-report.json"
if ! (cd "$dir" && cargo echoes \
  --threshold 1 \
  --min-tokens "$MIN_TOKENS" \
  --no-fail --json "$@") > "$base_report.raw" \
  || ! jq --rawfile prefix "$RUNNER_TEMP/echoes-prefix" -f "$RUNNER_TEMP/echoes-root.jq" \
    "$base_report.raw" > "$base_report"; then
  rm -f "$base_report"
  echo "echoes: the base scan failed, the full report is reviewed"
fi
rm -f "$base_report.raw"
