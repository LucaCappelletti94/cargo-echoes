#!/usr/bin/env bash
set -eo pipefail
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
selection=()
while read -r name; do
  [ -n "$name" ] && selection+=(--organization "$name")
done <<< "$ORGANIZATIONS"
while read -r name; do
  [ -n "$name" ] && selection+=(--repository "$name")
done <<< "$REPOSITORIES"
prefix=''
if [ "${#selection[@]}" -gt 0 ]; then
  # A job token reaches private repositories, the workflow token the public ones.
  export GH_TOKEN="${GH_TOKEN:-$WORKFLOW_TOKEN}"
else
  # Report paths are relative to the workspace root cargo resolves, the cwd or one of
  # its ancestors. GitHub reads, matches and links paths from the repository root, so
  # one prefix from that root projects every report path for the later steps.
  root="$(cargo metadata --no-deps --format-version 1 2>/dev/null | jq -r '.workspace_root')" || root=''
  if [ -n "$root" ] && [ -n "${GITHUB_WORKSPACE:-}" ]; then
    prefix="$(realpath --relative-to="$GITHUB_WORKSPACE" "$root" 2>/dev/null)" || prefix=''
    [ "$prefix" = "." ] && prefix=''
  fi
fi
printf '%s' "$prefix" > "$RUNNER_TEMP/echoes-prefix"
printf '%s\n' 'if $prefix == "" then . else ((.groups[]?.sites[]?, .context_groups[]?.sites[]?) |= (.file = "\($prefix)/\(.file)")) | (if .reuse then (.reuse.groups[]?.local |= (.file = "\($prefix)/\(.file)")) else . end) end' \
  > "$RUNNER_TEMP/echoes-root.jq"
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
# The base scan reads the same flags, so it sees the code the way this scan does.
: > "$RUNNER_TEMP/echoes-scan-args"
[ "$#" -eq 0 ] || printf '%s\0' "$@" > "$RUNNER_TEMP/echoes-scan-args"
set -- "$@" "${selection[@]}"
if [ -n "$PR_NUMBER" ] || [ "$DRY_RUN" = "true" ]; then
  report="$RUNNER_TEMP/echoes-report.json"
  status=0
  cargo echoes \
    --threshold "$THRESHOLD" \
    --min-tokens "$MIN_TOKENS" \
    --no-fail --json "$@" > "$report.raw" || status=$?
  if [ ! -s "$report.raw" ]; then
    echo "echoes: the scan produced no report"
    exit 2
  fi
  jq --arg prefix "$prefix" -f "$RUNNER_TEMP/echoes-root.jq" "$report.raw" > "$report"
  rm -f "$report.raw"
  ejq -r 'include "echoes"; stats' "$report"
  # The whole scan in the run summary, which needs no token, so a
  # fork pull request has it whatever the review can do.
  sum_append "$(
    ejq -r 'include "echoes"; "### echoes\n\n" + stats' "$report"
    ejq -r 'include "echoes";
      .groups[] | "\n- group `\(.id)`, \(.kind // "doctest"), \(.sites | length) copies\n"
        + ([.sites[] | site_md(""; "")] | join("\n"))' "$report"
  )"
  # Context groups and dependency reuse, grouped by local context with
  # the accessible APIs first, in the same public report.
  context_summary="$(ejq -r 'include "echoes";
    (if (.context_groups | length) > 0
     then "### Context groups\n\n"
          + ([ .context_groups[]
              | "- group `\(.id)`, \(.sites | length) copies\n"
                + ([ .sites[] | site_md(""; "") ] | join("\n"))
            ] | join("\n"))
     else "" end)
    + (if ((.reuse // {}) | (.groups // []) | length) > 0
       then "### Dependency reuse\n\n"
            + ([ (.reuse.groups // [])[]
                | "- local " + (.local | site_md(""; "") | ltrimstr("  - ")) + "\n"
                  + ([ .candidates[] | cand_md ] | join("\n"))
              ] | join("\n"))
            + "Dependency coverage: \(.reuse.coverage.packages) packages, \(.reuse.coverage.indexed_files) files indexed, \(.reuse.coverage.cached_files) from cache"
            + (if .reuse.coverage.complete then "" else "\n\nThe requested dependency analysis is incomplete, the run fails\n" + ([ .reuse.coverage.failures[] | "- " + . ] | join("\n")) end)
            + (if (.reuse.coverage.warnings | length) > 0 then "\n" + ([ .reuse.coverage.warnings[] | "- " + . ] | join("\n")) else "" end)
       else "" end)' "$report")"
  [ -n "$context_summary" ] && sum_append "$context_summary"
  if [ "$(jq '(.groups | length) + (.context_groups | length) + ((.reuse // {}).groups // [] | length)' "$report")" -gt 0 ]; then
    echo "found=true" >> "$GITHUB_OUTPUT"
  fi
  if [ "$CONTEXT_DEPTH" != "0" ]; then
    index_ok=false
    [ "$(jq -r '.reuse.coverage.complete // false' "$report")" = "true" ] && index_ok=true
    echo "index-ok=$index_ok" >> "$GITHUB_OUTPUT"
  fi
  exit "$status"
else
  out="$RUNNER_TEMP/echoes-gate.txt"
  status=0
  gate=()
  # Annotations anchor to this repository's files, which a selection does not scan.
  [ "$ANNOTATIONS" = "true" ] && [ "${#selection[@]}" -eq 0 ] && gate+=(--github)
  cargo echoes \
    --threshold "$THRESHOLD" \
    --min-tokens "$MIN_TOKENS" \
    "${gate[@]}" "$@" > "$out" || status=$?
  # GitHub resolves an annotation path from the repository root.
  if [ -n "$prefix" ]; then
    while IFS= read -r line; do
      case "$line" in
        ::*" file="*) printf '%s file=%s/%s\n' "${line%% file=*}" "$prefix" "${line#* file=}" ;;
        *) printf '%s\n' "$line" ;;
      esac
    done < "$out" > "$out.rooted"
    mv "$out.rooted" "$out"
  fi
  cat "$out"
  sum_append "$(
    echo '### echoes'
    echo
    echo '```'
    grep -v '^::' "$out"
    echo '```'
  )"
  if [ "$CONTEXT_DEPTH" != "0" ]; then
    case "$status" in
      0|1) echo "index-ok=true" >> "$GITHUB_OUTPUT" ;;
      *) echo "index-ok=false" >> "$GITHUB_OUTPUT" ;;
    esac
  fi
  exit "$status"
fi
