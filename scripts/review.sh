#!/usr/bin/env bash
set -eo pipefail
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# Each api call checks its own status and writes an outcome file, so a failed
# call degrades to a skipped review instead of aborting the run.
# Ends the step with the status $1 and the exit code $2, after printing the rest.
finish() {
  local outcome="$1" code="$2"
  shift 2
  [ "$#" -eq 0 ] || printf '%s\n' "$@"
  echo "$outcome" > "$status"
  exit "$code"
}
pr="$PR_NUMBER"
dry="$DRY_RUN"
repo="$GITHUB_REPOSITORY"
report="$RUNNER_TEMP/echoes-report.json"
base_report="$RUNNER_TEMP/echoes-base-report.json"
body="$RUNNER_TEMP/echoes-review.md"
status="$RUNNER_TEMP/echoes-post-status"
mode="$MODE"
payload="${PAYLOAD:-$RUNNER_TEMP/echoes-payload}"
comments="$RUNNER_TEMP/echoes-comments.ndjson"
reuse_tmp="$RUNNER_TEMP/echoes-reuse.ndjson"
blank_file="$RUNNER_TEMP/echoes-blank.txt"
dirs_file="$RUNNER_TEMP/echoes-dirs.json"
echo '{"owned": [], "foreign": []}' > "$dirs_file"
scope=''
: > "$comments"
mkdir -p "$payload"
failed_sites=''
failed_log=''
out_of_range=''
omitted=0
rate_omitted=0
omitted_file="$RUNNER_TEMP/echoes-omitted.txt"
run_url="$GITHUB_SERVER_URL/$repo/actions/runs/$GITHUB_RUN_ID"
# The prepared report rebuilt from its first JSON document, keeping only the fields
# the renderer reads. Each check below is one the renderer needs, a string it
# concatenates, a line it compares or a list it iterates or indexes. Picking a
# field out of a non-object is an error that rejects the report on its own.
# Values it only compares or prints, such as the kind and the remedy, pass
# through so a newer binary's values still render.
canonical_report() {
  jq -c -s '
    def int: type == "number" and . >= 1 and . <= 2147483647 and . == floor;
    def str: type == "string";
    def need(c): if c then . else error("malformed") end;
    def site(end_required):
      need((.file | str) and (.item | str) and (.line | int)
        and (if end_required then (.end | int) else (.end == null or (.end | int)) end)
        and (.self_type == null or (.self_type | str)))
      | {file, item, line, end, self_type, public};
    def candidate:
      need(.conditions == null or (.conditions | type == "array"))
      | {package, version, depth, conditions, action, api_path, site: {item: .site.item},
         source_url, issue_url, existing_issue: (.existing_issue | if . then {url, state} else null end),
         lookup_warning};
    .[0]
    | need((.groups | type == "array")
      and (.context_groups == null or (.context_groups | type == "array"))
      and ((.reuse // {}).groups == null or ((.reuse // {}).groups | type == "array")))
    | { groups: [.groups[]
          | need((.hash | str) and (.sites | type == "array" and length > 0))
          | {kind, remedy, hash, sites: [.sites[] | site(false)]}],
        context_groups: [(.context_groups // [])[]
          | need((.hash | str) and (.sites | type == "array" and length > 0))
          | {hash, sites: [.sites[] | site(true)]}],
        reuse: {groups: [((.reuse // {}).groups // [])[]
          | need((.hash | str) and (.candidates | type == "array"))
          | {hash, local: (.local | site(true)), candidates: [.candidates[] | candidate]}]} }
  ' "$1" > "$2" 2>/dev/null
}
base_sha="$(cat "$RUNNER_TEMP/echoes-base-sha" 2>/dev/null || echo '')"
head_sha=''
changed_files=''
if [ "$mode" = "post" ]; then
  # The payload comes from a job the pull request controls, so it is data. The
  # pull request is the one the triggering run built, or the job's pr-number.
  reject() { finish rejected 1 "echoes: $(escape_data "$1"), nothing is posted"; }
  stale() { finish skipped-stale 0 "echoes: $(escape_data "$1"), nothing is posted"; }
  for f in meta.json report.json; do
    [ -s "$payload/$f" ] || reject "the prepared review in $payload lacks $f, rerun the prepare job with this version of the action"
  done
  for f in meta.json report.json base-report.json blank.txt; do
    [ -e "$payload/$f" ] || continue
    size="$(wc -c < "$payload/$f" 2>/dev/null | tr -d ' ')" || size=''
    [[ "$size" =~ ^[0-9]+$ ]] && [ "$size" -le $((128 * 1024 * 1024)) ] \
      || reject "the prepared $f is not a file of at most 128 MiB"
  done
  meta="$RUNNER_TEMP/echoes-meta.json"
  jq -c -s '.[0] | if type == "object" then . else error("malformed") end' "$payload/meta.json" > "$meta" 2>/dev/null \
    || reject "the prepared metadata is malformed"
  claimed_pr="$(jq -r '.pr // empty | tostring' "$meta")"
  claimed_head="$(jq -r '.head_sha // empty | tostring' "$meta")"
  # One line only, grep reads a newline in its pattern as a second pattern.
  [[ "$claimed_pr" =~ ^[1-9][0-9]{0,9}$ ]] || reject "the prepared pull request number is not a number"
  if [ "$EVENT_NAME" = "workflow_run" ]; then
    [ "$RUN_EVENT" = "pull_request" ] || reject "the triggering run came from $RUN_EVENT, not from a pull request"
    # Only ids from the event, the pull request itself comes from the API.
    [[ "$RUN_HEAD_SHA" =~ ^[0-9a-f]{40}$ ]] || reject "the triggering run did not build a commit"
    [[ "$RUN_HEAD_REPO_ID" =~ ^[1-9][0-9]{0,19}$ ]] || reject "the triggering run has no head repository"
    open="$(gh api "repos/$repo/pulls?state=open&per_page=100" \
      --paginate --jq '.[]' 2>/dev/null)" || reject "the open pull requests cannot be listed"
    built="$(printf '%s\n' "$open" | jq -r --arg sha "$RUN_HEAD_SHA" --arg id "$RUN_HEAD_REPO_ID" \
      'select(.head.sha == $sha and (.head.repo.id | tostring) == $id) | .number' 2>/dev/null)"
    [ -n "$built" ] || stale "no open pull request has the head $RUN_HEAD_SHA the triggering run built"
    grep -qx -- "$claimed_pr" <<< "$built" || reject "the prepared review names pull request $claimed_pr, which the triggering run did not build"
    [ "$claimed_head" = "$RUN_HEAD_SHA" ] || reject "the prepared head differs from the head the triggering run built"
    [ -z "$pr" ] || [ "$pr" = "$claimed_pr" ] || reject "the prepared review names pull request $claimed_pr, the job names $pr"
    head_sha="$RUN_HEAD_SHA"
  else
    [ "$pr" = "$claimed_pr" ] || reject "outside workflow_run, the pr-number input must name the prepared pull request $claimed_pr"
    head_sha="$(gh api "repos/$repo/pulls/$pr" --jq '.head.sha' 2>/dev/null)" || head_sha=''
    [[ "$head_sha" =~ ^[0-9a-f]{40}$ ]] || reject "pull request $pr cannot be read"
    [ "$claimed_head" = "$head_sha" ] || stale "pull request $pr moved on from the prepared head $claimed_head"
  fi
  pr="$claimed_pr"
  canonical_report "$payload/report.json" "$report" || reject "the prepared report is malformed"
  rm -f "$base_report"
  if [ -e "$payload/base-report.json" ]; then
    canonical_report "$payload/base-report.json" "$base_report" || reject "the prepared base report is malformed"
  fi
  if [ -f "$payload/blank.txt" ]; then cp "$payload/blank.txt" "$blank_file"; else : > "$blank_file"; fi
  version="$(jq -r '.version // empty | tostring' "$meta")"
  base_sha="$(jq -r '.base_sha // empty | tostring' "$meta")"
  [[ "$base_sha" =~ ^[0-9a-f]{40}$ ]] || base_sha=''
  # The workspace the payload scanned, whose threads and reviews of earlier heads
  # this job may clean up. An absent workspace leaves them as they are.
  scope="$(jq -r 'if has("scope") then (.scope | if type == "string" then . else error("malformed") end) else "" end' "$meta" 2>/dev/null)" \
    || reject "the prepared workspace is malformed"
  if [ -n "$scope" ]; then
    [[ "$scope" =~ ^[A-Za-z0-9._@+/-]{1,512}$ ]] || reject "the prepared workspace is malformed"
  fi
  jq -c 'def dirs: if . == null then []
      elif type == "array" and length <= 10000 and all(.[]; type == "string" and test("^[A-Za-z0-9._@+/-]{1,512}$")) then .
      else error("malformed") end;
    {owned: (.owned_dirs | dirs), foreign: (.foreign_dirs | dirs)}' "$meta" > "$dirs_file" 2>/dev/null \
    || reject "the prepared workspace directories are malformed"
  changed_files="$(gh api "repos/$repo/pulls/$pr/files" --paginate --jq '.[].filename' 2>/dev/null)" \
    || reject "the changed files of pull request $pr cannot be listed"
  [ "$dry" != "true" ] || reject "dry-run does not apply to post mode"
else
  version="$(cargo echoes --version | awk '{ print $NF }')"
  # The head SHA and changed file list, empty when there is no pull request.
  if [ -n "$pr" ]; then
    head_sha="$(gh api "repos/$repo/pulls/$pr" --jq '.head.sha' 2>/dev/null)" || head_sha=''
    if [ -n "$head_sha" ]; then
      changed_files="$(gh api "repos/$repo/pulls/$pr/files" --paginate --jq '.[].filename' 2>/dev/null)" || changed_files=''
    fi
  fi
  if [ -n "$pr" ] && [ -z "$head_sha" ] && [ "$dry" != "true" ]; then
    finish failed 1 "echoes: cannot read pull request $pr, the review is not posted"
  fi
  # Blocks followed by an empty line, which a deletion suggestion removes too.
  jq -r '.groups[].sites[] | select(.end != null) | "\(.file)\t\(.end)"' "$report" | sort -u |
    while IFS=$'\t' read -r f e; do
      nx="$(awk -v l="$e" 'NR == l + 1 { print "x" $0; exit }' "$f" 2>/dev/null)"
      if [ -n "$nx" ] && [ -z "$(printf '%s' "${nx#x}" | tr -d '[:space:]')" ]; then
        printf '%s:%s\n' "$f" "$e"
      fi
    done > "$blank_file"
  # The workspace this run scanned, the root and member package directories it owns, and
  # every other directory with a tracked manifest, which another workspace owns.
  scope="$(cat "$RUNNER_TEMP/echoes-prefix" 2>/dev/null)"
  [ -n "$scope" ] || scope='.'
  [[ "$scope" =~ ^[A-Za-z0-9._@+/-]{1,512}$ ]] || scope=''
  if [ -n "$scope" ]; then
    owned="$( { printf '%s\n' "$scope"
      (cd "$GITHUB_WORKSPACE/$scope" 2>/dev/null && cargo metadata --no-deps --format-version 1 2>/dev/null) |
        jq -r '.packages[]?.manifest_path' 2>/dev/null |
        while IFS= read -r m; do realpath -m --relative-to="$GITHUB_WORKSPACE" "$(dirname "$m")" 2>/dev/null; done
    } | sort -u)" || true
    tracked="$(git -C "$GITHUB_WORKSPACE" ls-files -- '*Cargo.toml' 2>/dev/null |
      while IFS= read -r f; do [ "${f##*/}" != Cargo.toml ] || dirname "$f"; done | sort -u)" || tracked=''
    jq -n --arg owned "$owned" --arg tracked "$tracked" '
      def lines: split("\n") | map(select(test("^[A-Za-z0-9._@+/-]{1,512}$")));
      ($owned | lines) as $o | {owned: $o, foreign: (($tracked | lines) - $o)}' > "$dirs_file"
  fi
fi
onlynew=0
scope_uri=''
[ -n "$scope" ] && scope_uri="$(uri "$scope")"
oor_local=0
[ -s "$base_report" ] && onlynew=1
if [ "$dry" = "true" ]; then
  # A dry run renders the review as if every scanned site were in
  # the diff, so the step summary shows each comment in full.
  changed_json="$(ejq 'include "echoes"; [report_sites | .s.file] | unique' "$report")"
elif [ -n "$pr" ] && [ -n "$head_sha" ] && [ -n "$changed_files" ]; then
  changed_json="$(printf '%s\n' "$changed_files" | jq -R . | jq -s .)"
else
  changed_json='[]'
fi

# Base site identity, a site exists on the base when the same hash, item and
# path were already there, and the first base copy is the kept original.
if [ "$onlynew" = "1" ]; then
  base_json="$(ejq 'include "echoes";
    { keys: [report_sites | .h as $h | .s | site_key($h)],
      keep: (first_sites(.groups) | map_values(loc)),
      keep_ctx: (first_sites(.context_groups // []) | map_values(loc)),
      keep_end: (first_sites(.groups) | map_values(.end // null)),
      keep_item: (first_sites(.groups) | map_values(.item)) }' "$base_report")"
else
  base_json='{"keys": [], "keep": {}, "keep_ctx": {}, "keep_end": {}, "keep_item": {}}'
fi

# Sites to comment on, one JSON row per site of every kind. In only-new mode a site
# must be introduced by this pull request, which holds more head copies of its hash,
# item and path, up to and including this one, than the base holds. A site is inline
# when its file is in the pull request diff. The kept copy of a group stays silent,
# its first site, or in only-new mode the first added site when the base lacks the
# group. A reuse finding is advice on its one local site, which always speaks.
onlynew_mode=false
[ "$onlynew" = "1" ] && onlynew_mode=true
# One command line argument holds at most 128 KiB, a large pull request's
# file list exceeds it, so jq reads both lists from files.
changed_file="$RUNNER_TEMP/echoes-changed.json"
base_file="$RUNNER_TEMP/echoes-base-keys.json"
rows_file="$RUNNER_TEMP/echoes-rows.ndjson"
printf '%s' "$changed_json" > "$changed_file"
printf '%s' "$base_json" > "$base_file"
ejq -c --slurpfile changed "$changed_file" --slurpfile base "$base_file" \
  --argjson onlynew_mode "$onlynew_mode" --arg sha "$head_sha" \
  --arg blob "$GITHUB_SERVER_URL/$repo/blob" 'include "echoes";
  def site_rows($h; $keys):
    [ .sites[] | site_key($h) ] as $ks
    | [ .sites | to_entries[]
        | (.value | site_key($h)) as $k
        | { c: (.key + 1), s: .value,
            isNew: ((([$ks[: .key][] | select(. == $k)] | length) + 1) > ([$keys[] | select(. == $k)] | length)) } ]
    | (map(select(.isNew)) | length) as $knew
    | map(. + {knew: $knew});
  def group_fields: {n: (.sites | length), first: (.sites[0] | loc), fend: (.sites[0].end // null)};
  $changed[0] as $changed
  | $base[0] as $base
  | [ ( range(0; (.groups | length)) as $gi | .groups[$gi]
        | (.kind // "doctest") as $kind
        | ([.sites[].self_type] as $t | ($t[0]) as $f | ($t[1:] | map(. != $f) | any)) as $spans
        # A binary older than 0.4.1 sends no remedy, its rule only told types apart.
        | (if $kind == "function" then (.remedy // (if $spans then "generic-or-macro" else "delete" end)) else "delete" end) as $remedy
        | (group_fields + { cls: "local", kind: $kind, g: ($gi + 1), hash, remedy: $remedy,
            selftypes: (reduce (.sites[] | (.self_type // "free function")) as $t ([]; if index($t) == null then . + [$t] else . end) | join(", ")),
            firstitem: .sites[0].item, basekeep: ($base.keep[.hash] // ""), baseitem: ($base.keep_item[.hash] // ""),
            bend: ($base.keep_end[.hash] // null), label: "group \($gi + 1)", issue_sites: ([.sites[] | site_line] | capped) }) as $group
        | site_rows(.hash; $base.keys)[] | $group + . )
    , ( (.context_groups // [])[] as $gr
        | (($gr | group_fields) + { cls: "context", kind: "context", hash: $gr.hash, basekeep: ($base.keep_ctx[$gr.hash] // ""),
            label: "context group \($gr.hash[:8])", issue_sites: ([$gr.sites[] | site_line] | capped) }) as $group
        | $gr | site_rows($gr.hash; $base.keys)[]
        | . as $row
        | $group + . + {others: [$gr.sites | to_entries[] | select(.key != $row.c - 1) | .value | site_md($sha; $blob)]} )
    , ( ((.reuse // {}).groups // [])[] as $gr
        | (({sites: [$gr.local]} | group_fields) + { cls: "reuse", kind: "reuse", hash: $gr.hash, basekeep: "",
            label: "reuse group \($gr.hash[:8])",
            issue_sites: ([($gr.local | site_line)] + [$gr.candidates[] | "\(.package) \(.version)  \(.site.item)"] | capped),
            candidates_md: ([$gr.candidates[] | cand_md] | join("\n")) }) as $group
        | {sites: [$gr.local]} | site_rows($gr.hash; $base.keys)[] | $group + . )
    ]
  | map(. as $r | select(($changed | index($r.s.file)) != null) | select(if $onlynew_mode then .isNew else true end))
  | . as $rows
  | (reduce range(0; length) as $i ({}; ($rows[$i] | "\(.cls)\t\(.hash)") as $gk | if has($gk) then . else .[$gk] = $i end)) as $firsts
  | range(0; length) as $i
  | $rows[$i] | $firsts["\(.cls)\t\(.hash)"] as $fi
  | . + (if $onlynew_mode
         then {removable: (.cls == "reuse" or .basekeep != "" or $fi != $i), keepat: ($rows[$fi].s | loc), keepat_end: ($rows[$fi].s.end // null)}
         else {removable: (.cls == "reuse" or .c != 1), keepat: .first, keepat_end: .fend} end)
  | select(.removable)
  | . + {path: .s.file, line: .s.line, end: (.s.end // null), item: .s.item, key: ("\(.kind):\(.hash):\(.s.file):\(.s.item)" | @uri)}
' "$report" > "$rows_file"

# The inline posts, live mode only. The first post is the permission probe.
# Each comment links to a prefilled wrong-review issue naming its label and sites.
report_link() {
  printf '[👎 Report a wrong review](https://github.com/LucaCappelletti94/cargo-echoes/issues/new?template=wrong-review.yml&title=%s&version=%s&review=%s&group=%s)' \
    "$(uri "Wrong review on $repo#$pr, $1")" \
    "$(uri "$version")" \
    "$(uri "$GITHUB_SERVER_URL/$repo/pull/$pr")" \
    "$(uri "$2")"
}
support_link='[❤️ Support echoes](https://github.com/sponsors/LucaCappelletti94)'
# The hidden marks that tie a comment to its workspace and finding, and a review to
# its workspace, so a later head finds what this run posted.
thread_mark() { printf '<!-- echoes:v1 scope=%s key=%s -->' "$scope_uri" "$1"; }
review_mark() { printf '<!-- echoes:v1 scope=%s review -->' "$scope_uri"; }
# This workspace's earlier threads and reviews, cleaned up once the run knows its findings.
# $1 is the review this run posted, which stays.
cleanup() {
  local keep="${1:-}" id err refused=0 n=0 message reviews
  if [ -z "$scope" ]; then
    echo "echoes: the review names no workspace, the threads and reviews of earlier commits are left as they are"
    return 0
  fi
  if [ "$threads_ok" = "1" ]; then
    while IFS= read -r id; do
      [ -n "$id" ] || continue
      if ! err="$(gh api graphql -f query='mutation($threadId: ID!) { resolveReviewThread(input: {threadId: $threadId}) { thread { isResolved } } }' \
        -F "threadId=$id" 2>&1)"; then
        if printf '%s' "$err" | grep -qiE 'not accessible|FORBIDDEN|HTTP 403'; then
          refused=1
          break
        fi
        printf 'echoes: resolving thread %s failed\n' "$(escape_data "$id")"
        printf '%s\n' "$err" | sed 's/^/echoes: /'
        continue
      fi
      n=$((n + 1))
    done < "$gone_file"
    [ "$n" -gt 0 ] && echo "echoes: resolved $n review threads whose finding is gone"
    [ "$refused" = "1" ] && echo "echoes: resolving a review thread needs contents: write on this job, the threads of findings that are gone stay open"
  fi
  if ! reviews="$(gh api "repos/$repo/pulls/$pr/reviews" --paginate --jq '.[] | @json' 2>/dev/null)"; then
    echo "echoes: the reviews cannot be listed, earlier requests for changes stay"
    return 0
  fi
  message="No echoes finding of this workspace remains at $head_sha."
  [ -n "$keep" ] && message="Superseded by the echoes review of $head_sha."
  n=0
  refused=0
  while IFS= read -r id; do
    [ -n "$id" ] || continue
    if ! err="$(gh api -X PUT "repos/$repo/pulls/$pr/reviews/$id/dismissals" -f "message=$message" -f event=DISMISS 2>&1)"; then
      if is_forbidden "$err"; then
        refused=1
        break
      fi
      printf 'echoes: dismissing review %s failed\n' "$id"
      printf '%s\n' "$err" | sed 's/^/echoes: /'
      continue
    fi
    n=$((n + 1))
  done < <(printf '%s\n' "$reviews" | ejq -r --slurpfile dirs "$dirs_file" --arg mark "$(review_mark)" --arg keep "$keep" 'include "echoes";
    select(type == "object" and .user.login == "github-actions[bot]" and .state == "CHANGES_REQUESTED" and (.id | tostring) != $keep)
    | (.body // "") as $b
    | select(($b | contains($mark))
        or (($b | contains("<!-- echoes:v1") | not)
            and ($b == "" or ($b | startswith("These duplicates are not on lines"))
                 or ($b | startswith("Duplicates echoes could not comment inline"))
                 or ($b | test("^[0-9]+ inline copies were not posted")))
            and (([$b | match("`([^`]+):[0-9]+(-[0-9]+)?`").captures[0].string] | .[0]) as $path
                 | ($path == null or ($path | owned($dirs[0]))))))
    | .id | tostring | select(test("^[0-9]{1,20}$"))')
  [ "$n" -gt 0 ] && echo "echoes: dismissed $n earlier echoes reviews of this workspace"
  [ "$refused" = "1" ] && echo "echoes: dismissing a review needs pull-requests: write on this job, earlier requests for changes stay"
  return 0
}
rendered=''
reuse_rendered=''
findings=''
findings_span=0
: > "$reuse_tmp"
echoes_link='[echoes](https://github.com/LucaCappelletti94/cargo-echoes)'
# The local note of a doctest or function copy, its preview and its suggestion,
# appended to $comment. Returns 1 when the copy has no end line, a block spanning
# an include or an escaped literal or an older binary, which only the review body lists.
local_comment() {
  # A public copy that already shipped is called or deprecated, deleting it breaks callers.
  if [ "$remedy" = "delete" ] && [ "$kind" = "function" ] && [ "$public" = "1" ] && [ "$onlynew" != "1" ]; then
    remedy=public
  fi
  kept_item="$firstitem"
  [ -n "$baseitem" ] && kept_item="$baseitem"
  # The annotation carries the same finding without a token, so a
  # fork pull request shows it anchored in the diff. The post job's run
  # is not the pull request's, so it annotates nothing.
  if [ "$ANNOTATIONS" = "true" ] && [ "$dry" != "true" ]; then
    keep="$basekeep"
    [ -z "$keep" ] && keep="$keepat"
    annotate=1
    [ "$mode" = "post" ] && annotate=0
    if [ "$kind" = "function" ]; then
      case "$remedy" in
        generic-or-macro) where='on another type' share='a generic or a macro can share it' ;;
        helper-or-macro) where="and can't be deleted" share='a helper or a macro can share it' ;;
        merge-cfg) where='under another cfg' share='one function under both cfgs can replace them' ;;
        public) where='and is public API' share='make it call the kept copy or deprecate it' ;;
        *) where='' share='' ;;
      esac
      if [ -n "$where" ]; then
        [ "$annotate" = "1" ] && printf '::error file=%s,line=%s,title=echoes::%s repeats %s at %s %s, %s, group %s\n' \
          "$(escape_property "$p")" "$line" "$(escape_data "$item")" "$(escape_data "$kept_item")" "$(escape_data "$keep")" "$where" "$share" "$g"
        findings="$findings- $(md_code "$p:$line")  $(md_code "$item"), repeats $(md_code "$kept_item") at $(md_code "$keep") $where"$'\n'
        findings_span=1
      else
        [ "$annotate" = "1" ] && printf '::error file=%s,line=%s,title=echoes::%s duplicates the function %s at %s, group %s\n' \
          "$(escape_property "$p")" "$line" "$(escape_data "$item")" "$(escape_data "$kept_item")" "$(escape_data "$keep")" "$g"
        findings="$findings- $(md_code "$p:$line")  $(md_code "$item"), duplicates the function $(md_code "$kept_item") at $(md_code "$keep")"$'\n'
      fi
    else
      [ "$annotate" = "1" ] && printf '::error file=%s,line=%s,title=echoes::%s duplicates the doctest at %s, group %s\n' \
        "$(escape_property "$p")" "$line" "$(escape_data "$item")" "$(escape_data "$keep")" "$g"
      findings="$findings- $(md_code "$p:$line")  $(md_code "$item"), duplicates $(md_code "$keep")"$'\n'
    fi
  fi
  if [ -z "$end" ]; then
    out_of_range="$out_of_range- group $g  $(blob_link "$head_sha" "$p" "$line")  $(md_code "$item")"$'\n'
    oor_local=1
    return 1
  fi
  # The suggestion deletes the block plus the empty line right after
  # it, which would become a stray once the copy is gone.
  stop="$end"
  if [ "$remedy" = "delete" ] && grep -Fxq -- "$p:$end" "$blank_file" 2>/dev/null; then
    stop="$(awk -v l="$end" 'BEGIN { print l + 1 }')"
  fi
  # The kept copy, on the base when the base already had the group.
  kept_sha="$head_sha" kept="$keepat" kept_end="$keepat_end"
  if [ "$onlynew" = "1" ] && [ -n "$basekeep" ]; then
    kept_sha="$base_sha" kept="$basekeep" kept_end="$bend"
  fi
  kept_line="${kept##*:}"
  url="$(blob_url "$kept_sha" "${kept%:*}" "$kept_line" "$kept_end")"
  loc="$(loc_md "$(loc_text "${kept%:*}" "$kept_line" "$kept_end")" "$url")"
  preview='the duplicated doctest'
  if [ "$kind" = "function" ]; then
    preview='the duplicated function'
    [ "$remedy" != "delete" ] && preview='the repeated function'
  fi
  kept_md="$(md_code "$kept_item")"
  if [ "$remedy" = "generic-or-macro" ]; then
    note="This method repeats $kept_md at $loc on another type ($(md_code "$selftypes")). A generic or a macro can share one body."
  elif [ "$remedy" = "helper-or-macro" ]; then
    note="This function repeats $kept_md at $loc. A trait or an exported symbol needs it, so it can't simply be deleted, a helper or a macro can share one body."
  elif [ "$remedy" = "public" ]; then
    note="This function repeats $kept_md at $loc. It is public API, so deleting it breaks its callers, make it call the kept copy or deprecate it."
  elif [ "$remedy" = "merge-cfg" ]; then
    note="This function repeats $kept_md at $loc under another \`cfg\`. Deleting either copy drops a configuration, one function under both conditions can replace them."
  elif [ "$kind" = "function" ]; then
    if [ "$onlynew" = "1" ] && [ -n "$basekeep" ]; then
      note="This function was added by this pull request and duplicates a copy that already exists at $loc, $echoes_link suggests deleting the added copy, or updating it if it was meant to differ."
    elif [ "$onlynew" = "1" ]; then
      note="This pull request adds $knew copies of the same function, $echoes_link suggests keeping the first added copy at $loc and deleting this one, or updating it if it was meant to differ."
    else
      note="This function appears $n times in this module, this is copy $c. $echoes_link suggests keeping the first copy at $loc and deleting this one, or updating it if it was meant to differ."
    fi
  elif [ "$onlynew" = "1" ] && [ -n "$basekeep" ]; then
    note="This doctest was added by this pull request and duplicates a copy that already exists at $loc, $echoes_link suggests removing the added copy."
  elif [ "$onlynew" = "1" ]; then
    note="This pull request adds $knew copies of the same doctest, $echoes_link suggests keeping the first added copy at $loc and removing this one."
  else
    note="This doctest appears $n times in this scan, this is copy $c. $echoes_link suggests keeping the first copy at $loc and removing this one."
  fi
  comment="$note"
  [ -n "$url" ] && comment="$comment"$'\n\n'"$(preview_of "$url" "$kept_line" "$kept_end" "$preview")"
  if [ "$remedy" = "delete" ]; then
    comment="$comment"$'\n\n```suggestion\n```'
  fi
  return 0
}
# The neutral note of a context or a reuse finding, approximate and upstream
# matches stay advice, so it carries no removal suggestion.
advice_comment() {
  if [ "$kind" = "context" ]; then
    note="This context is repeated in this scan. A shared helper can carry it once. The other copies are at"
    [ -n "$others" ] && note="$note"$'\n'"$others"
  else
    note="This context matches an upstream dependency context. The accessible APIs are listed first"
    note="$note"$'\n'"$candidates_md"
  fi
  note="$note"$'\n\n'"Type compatibility and behavior equivalence need human review."
  stop="$end"
  url="$(blob_url "$head_sha" "$p" "$line" "$end")"
  comment="$note"
  [ -n "$url" ] && comment="$comment"$'\n\n'"$(preview_of "$url" "$line" "$end" "the local context")"
  return 0
}
# One pass builds every comment, live posts them, the dry run renders them. Every
# comment ends with the same footer, and the local ones are posted first. Each row's
# fields are read in one jq call, shell-quoted by @sh, into these variables.
cls='' kind='' p='' line='' end='' item='' c='' n='' g='' basekeep='' baseitem='' bend='' knew=''
remedy='' firstitem='' selftypes='' public='' keepat='' keepat_end='' label='' issue_sites='' key=''
others='' candidates_md=''
while IFS= read -r row; do
  [ -n "$row" ] || continue
  eval "$(printf '%s' "$row" | jq -r '@sh "cls=\(.cls) kind=\(.kind) p=\(.path) line=\(.line) end=\(.end // "") item=\(.item) c=\(.c) n=\(.n) g=\(.g // "") basekeep=\(.basekeep) baseitem=\(.baseitem // "") bend=\(.bend // "") knew=\(.knew) remedy=\(.remedy // "") firstitem=\(.firstitem // "") selftypes=\(.selftypes // "") public=\(if .s.public then 1 else 0 end) keepat=\(.keepat) keepat_end=\(.keepat_end // "") label=\(.label) issue_sites=\(.issue_sites) key=\(.key) others=\(.others // [] | join("\n")) candidates_md=\(.candidates_md // "")"')"
  if [ "$cls" = "local" ]; then
    local_comment || continue
  else
    advice_comment
  fi
  comment="$comment"$'\n\n'"$(report_link "$label" "$issue_sites") $support_link"
  if [ "$dry" = "true" ]; then
    if [ "$cls" = "local" ]; then rendered+="$comment"$'\n\n'; else reuse_rendered+="$comment"$'\n\n'; fi
    continue
  fi
  [ -n "$scope" ] && comment="$comment"$'\n\n'"$(thread_mark "$key")"
  # One row per comment, so a later job can post it without the
  # source tree it was computed from.
  out="$reuse_tmp"
  [ "$cls" = "local" ] && out="$comments"
  jq -cn --arg cls "$cls" --arg path "$p" --argjson start "$line" --argjson stop "$stop" --arg group "$g" \
    --arg item "$item" --arg kind "$kind" --arg close "$end" --arg key "$key" --arg body "$comment" \
    '{path: $path, start_line: $start, line: $stop}
     + (if $cls == "local" then {group: $group, item: $item, close_line: ($close | tonumber)} else {item: $item, kind: $kind} end)
     + {key: $key, body: $body}' >> "$out"
done < "$rows_file"

if [ "$dry" = "true" ]; then
  stats="$(ejq -r 'include "echoes"; stats' "$report")"
  groups_block="$(jq -r '
    .groups | to_entries | map(
        "Group \(.key + 1), \(.value.kind // "doctest"), \(.value.sites | length) copies\n\n"
        + "```\n\(.value.sites[0].code)\n```\n\n"
        + (.value.sites | to_entries | map(
            (if .key == 0 then "first  " else "copy   " end)
            + .value.file + ":" + (.value.line | tostring) + "  " + .value.item
            + (if (.value.info | length) > 0 then "   (" + (.value.info | join(", ")) + ")" else "" end)
          ) | join("\n")) + "\n\n"
    ) | join("")' "$report")"
  {
    title='Duplicated doctests found by echoes'
    if [ "$(jq '[.groups[] | select(.kind == "function")] | length' "$report")" -gt 0 ]; then
      title='Duplicates found by echoes'
    fi
    printf '%s\n' "$title"
    echo 'echoes dry run, the review is not posted'
    echo
    printf '%s\n' "$stats"
    echo
    printf '%s\n\n' "$groups_block"
    echo 'Inline comments echoes would post'
    echo
    printf '%s' "$rendered"
    if [ -n "$reuse_rendered" ]; then
      echo
      echo 'Neutral reuse and context advice echoes would post'
      echo
      printf '%s' "$reuse_rendered"
    fi
    if [ -n "$out_of_range" ]; then
      echo
      echo 'Sites outside the lines GitHub shows in the diff, the review body would list them'
      echo
      printf '%s' "$out_of_range"
    fi
    echo
    echo
    echo "Run $run_url"
  } > "$body"
  sum_append "$(cat "$body")"
  echo "echoes dry run, the review is not posted"
  cat "$body"
  finish dry-run 0
fi

# Prepare hands the posting job data only, which that job checks and renders
# again, so nothing a pull request writes into the payload is posted as is.
if [ "$mode" = "prepare" ]; then
  jq -n --arg pr "$pr" --arg head "$head_sha" --arg base "$base_sha" --arg version "$version" \
    --arg scope "$scope" --slurpfile dirs "$dirs_file" \
    '{pr: $pr, head_sha: $head, base_sha: $base, version: $version,
      scope: $scope, owned_dirs: $dirs[0].owned, foreign_dirs: $dirs[0].foreign}' > "$payload/meta.json"
  cp "$report" "$payload/report.json"
  [ "$onlynew" = "1" ] && cp "$base_report" "$payload/base-report.json"
  cp "$blank_file" "$payload/blank.txt"
fi
# The threads of earlier heads. A finding with a thread of this workspace, open or
# resolved, or with an open thread from before the marks on its lines, is not posted
# again, and the open threads whose finding is gone are resolved once the run is done.
threads_ok=0
gone_file="$RUNNER_TEMP/echoes-gone.txt"
: > "$gone_file"
touch "$comments" "$reuse_tmp"
local_found="$oor_local"
[ -s "$comments" ] && local_found=1
found_n="$(cat "$comments" "$reuse_tmp" | wc -l)"
if [ "$mode" != "prepare" ] && [ -n "$scope" ] && [ -n "$pr" ]; then
  threads_raw="$RUNNER_TEMP/echoes-threads.ndjson"
  threads="$RUNNER_TEMP/echoes-threads.json"
  rows_all="$RUNNER_TEMP/echoes-rows.json"
  if list_threads "$repo" "$pr" > "$threads_raw" 2>/dev/null; then
    threads_ok=1
    ejq -s --slurpfile dirs "$dirs_file" --arg scope "$scope_uri" 'include "echoes";
      map(select(type == "object")
        | (.comments.nodes[0] // {}) as $c | ($c.body // "" | tostring) as $b
        | select(($c.author.login // "") == "github-actions")
        | ([$b | capture("<!-- echoes:v1 scope=(?<s>[^ ]+) key=(?<k>[^ ]+) -->")] | .[0]) as $m
        | if $m != null then (if $m.s == $scope then {id, resolved: (.isResolved == true), key: $m.k} else empty end)
          elif ($b | legacy_class) != null and ((.path // "" | tostring) | owned($dirs[0]))
          then {id, resolved: (.isResolved == true), legacy: ($b | legacy_class), path, line}
          else empty end)' "$threads_raw" > "$threads"
    cat "$comments" "$reuse_tmp" | jq -s 'map({key, cls: (if .group then "local" else .kind end), path, start_line, line})' > "$rows_all"
    jq -r --slurpfile rows "$rows_all" '
      def covers($t): .path == $t.path and .cls == $t.legacy and ($t.line | type) == "number"
        and .start_line <= $t.line and $t.line <= .line;
      $rows[0] as $r | .[] | select(.resolved | not)
      | select(if .key then (.key as $k | $r | any(.[]; .key == $k) | not)
               else (. as $t | $r | any(.[]; covers($t)) | not) end)
      | .id' "$threads" > "$gone_file"
    for f in "$comments" "$reuse_tmp"; do
      jq -c --slurpfile t "$threads" '
        (if .group then "local" else .kind end) as $cls | . as $row
        | select($t[0] | any(.[]; .key and .key == $row.key) | not)
        | select($t[0] | any(.[]; .legacy and (.resolved | not) and .path == $row.path and .legacy == $cls
            and (.line | type) == "number" and $row.start_line <= .line and .line <= $row.line) | not)' "$f" > "$f.new"
      mv "$f.new" "$f"
    done
  else
    echo "echoes: the review threads cannot be listed, findings that already have a thread may be posted again and no thread is resolved"
  fi
fi
# The inline posts are capped per run, the omitted copies go to the
# review body and the run summary.
case "$MAX_COMMENTS" in
  '' | *[!0-9]*) max_comments=50 ;;
  *) max_comments="$MAX_COMMENTS" ;;
esac
: > "$omitted_file"
touch "$comments" "$reuse_tmp"
local_n="$(wc -l < "$comments")"
reuse_n="$(wc -l < "$reuse_tmp")"
total_n=$((local_n + reuse_n))
unposted_list() {
  ejq -rs 'include "echoes"; map("- " + ((.path + ":" + (.start_line | tostring)) | md_code) + "  " + (.item | md_code)
    + (if .group then " (group " + (.group | tostring) + ")" else " (" + (.kind // "context") + ")" end))
    | join("\n")'
}
if [ "$total_n" -gt "$max_comments" ]; then
  omitted=$((total_n - max_comments))
  keep_local="$max_comments"
  [ "$keep_local" -gt "$local_n" ] && keep_local="$local_n"
  keep_reuse=$((max_comments - keep_local))
  { tail -n +$((keep_local + 1)) "$comments"
    tail -n +$((keep_reuse + 1)) "$reuse_tmp"; } | unposted_list > "$omitted_file"
  head -n "$keep_local" "$comments" > "$comments.capped"
  head -n "$keep_reuse" "$reuse_tmp" > "$reuse_tmp.capped"
  mv "$comments.capped" "$comments"
  mv "$reuse_tmp.capped" "$reuse_tmp"
fi
# Nothing to post. In prepare mode the payload still exists so the posting job has
# something to download. A posting run cleans up, and goes on to renew its request
# for changes when local copies remain whose threads already exist.
if [ ! -s "$comments" ] && [ -z "$out_of_range" ] && [ ! -s "$reuse_tmp" ] && [ "$omitted" -eq 0 ] \
  && { [ "$mode" = "prepare" ] || [ "$local_found" != "1" ]; }; then
  if [ "$found_n" -gt 0 ]; then
    outcome=skipped-existing message="echoes: every finding of this pull request already has a review thread, nothing new is posted"
  elif [ "$onlynew" = "1" ]; then
    outcome=skipped-existing message="echoes: no duplicate was introduced by this pull request, the review is not posted"
  else
    outcome=skipped-nodiff message="echoes: no duplicate is in the pull request diff, the review is not posted"
  fi
  echo "$message"
  [ "$mode" = "prepare" ] || cleanup
  finish "$outcome" 0
fi
# The reported copies in the run summary, beside the whole scan the
# scan step wrote there.
if [ -n "$findings" ] && [ "$dry" != "true" ]; then
  findings_title='#### Copies to remove'
  [ "$findings_span" = "1" ] && findings_title='#### Findings'
  sum_append "$(printf '\n%s\n\n%s' "$findings_title" "$findings")"
fi
# The omitted copies and the rate limit stay in the run summary, the
# public record of what the review did not post inline.
if [ "$omitted" -gt 0 ]; then
  omitted_block="$(printf '#### Inline copies not posted\n\n%s inline copies were not posted, they are listed in this run summary\n\n' "$omitted")"
  [ -s "$omitted_file" ] && omitted_block="$omitted_block$(head -c 400000 "$omitted_file")"$'\n'
  sum_append "$omitted_block"
fi
if [ "$mode" = "prepare" ]; then
  finish prepared 0 "echoes: the review is prepared in $payload for a later job"
fi

readonly_notice() {
  echo "echoes: the GITHUB_TOKEN is read-only for this pull request, the review is not posted"
  echo "echoes: the annotations and the run summary carry the findings, the removal suggestion needs a review comment"
}
# The first post is the permission probe.
readonly=0
rate_limited=0
posted_local=0
posted_reuse=0
planned_n="$(cat "$comments" "$reuse_tmp" | wc -l)"
post_one() {
  local row="$1" p line stop err item g close loc
  eval "$(printf '%s' "$row" | jq -r '@sh "p=\(.path) line=\(.start_line) stop=\(.line) item=\(.item) g=\(.group // .kind) close=\(.close_line // .line)"')"
  if ! err="$(printf '%s' "$row" | jq -r '.body' | gh api "repos/$repo/pulls/$pr/comments" \
    -f "commit_id=$head_sha" -f "path=$p" \
    -F "start_line=$line" -F "line=$stop" \
    -f start_side=RIGHT -f side=RIGHT -F 'body=@-' 2>&1)"; then
    if rate_limited_error "$err"; then
      rate_limited=1
      return 1
    fi
    if is_forbidden "$err"; then
      readonly=1
      return 1
    fi
    printf 'echoes: %s:%s %s\n' "$(escape_data "$p")" "$line" "$(escape_data "$item")"
    printf '%s\n' "$err" | sed 's/^/echoes: /'
    loc="$(blob_link "$head_sha" "$p" "$line" "$close")"
    if printf '%s' "$err" | grep -qE 'HTTP 422|"status": "422"'; then
      out_of_range="$out_of_range- group $g  $loc  $(md_code "$item")"$'\n'
    else
      failed_sites="$failed_sites- group $g  $loc  $(md_code "$item")"$'\n'
      failed_log="$failed_log  group $g  $(escape_data "$p"):$line  $(escape_data "$item")"$'\n'
    fi
  fi
  return 0
}
# Posts the rows of file $1 until one fails for good, counting them in the variable named $2.
post_rows() {
  local -n posted="$2"
  while IFS= read -r row; do
    [ -n "$row" ] || continue
    post_one "$row" || break
    posted=$((posted + 1))
  done < "$1"
}
post_rows "$comments" posted_local
if [ "$readonly" != "1" ] && [ "$rate_limited" != "1" ]; then
  post_rows "$reuse_tmp" posted_reuse
fi
if [ "$readonly" = "1" ]; then
  readonly_notice
  finish skipped-readonly 0
fi
# The rows the rate limit left unposted, listed like the omitted ones.
if [ "$rate_limited" = "1" ]; then
  rate_omitted=$((planned_n - posted_local - posted_reuse))
  rate_unposted="$(
    { tail -n +$((posted_local + 1)) "$comments"
      tail -n +$((posted_reuse + 1)) "$reuse_tmp"; } | unposted_list
  )"
  echo "echoes: the api rate limit was reached, $rate_omitted inline copies were not posted"
fi
if [ "$rate_omitted" -gt 0 ]; then
  rate_block="$(printf '#### Rate limit\n\n%s inline copies were not posted because the api rate limit was reached, they are listed in this run summary\n\n' "$rate_omitted")"
  rate_block="$rate_block$rate_unposted"$'\n'
  sum_append "$rate_block"
fi
# A review with no comment of its own needs a body. Sites GitHub could not
# anchor inline are listed with permalinks to the head commit.
review_event='REQUEST_CHANGES'
[ "$local_found" = "1" ] || review_event='COMMENT'
review=(-f "event=$review_event")
# One list of the review body, its header and the lines that fit in $3 bytes, with a
# pointer to the run summary for the rest.
body_section() {
  [ -n "$2" ] || return 0
  local kept more
  kept="$(printf '%s' "$2" | fit_lines "$3")"
  more=$(($(printf '%s' "$2" | wc -l) - $(printf '%s' "$kept" | wc -l)))
  printf '%s\n\n%s' "$1" "$kept"
  [ "$more" -gt 0 ] && printf '\n%s' "and $more more, they are in the run summary $run_url"
  return 0
}
# The review body, each list within $1 bytes.
review_text() {
  local text
  text="$(body_section 'These duplicates are not on lines GitHub shows in this pull request diff, the inline comment cannot attach there. The links point at the copies on the head commit.' "$out_of_range" "$1")"
  [ -n "$failed_sites" ] && text="$text"$'\n\n'"$(body_section 'Duplicates echoes could not comment inline' "$failed_sites" "$1")"
  [ "$omitted" -gt 0 ] && text="$text"$'\n\n'"$omitted inline copies were not posted, they are listed in the run summary $run_url"
  [ "$rate_omitted" -gt 0 ] && text="$text"$'\n\n'"$rate_omitted inline copies were not posted because the api rate limit was reached, they are listed in the run summary $run_url"
  printf '%s' "$text"
}
review_body="$(review_text 1000000000)"
# The api rejects a review body over 65,536 characters, so the lists
# are truncated with a pointer to the run summary.
if [ "${#review_body}" -gt 65000 ]; then
  review_body="$(review_text $((65000 / 2 - 400)))"
fi
if [ -z "$review_body" ]; then
  if [ "$posted_local" -eq 0 ] && [ "$posted_reuse" -eq 0 ]; then
    review_body='The duplicates echoes reported on earlier commits are still in this pull request, their review threads stay open.'
  elif [ "$review_event" = "COMMENT" ]; then
    review_body='echoes found repeated code in this pull request, the inline comments name it.'
  else
    review_body='echoes found duplicates in this pull request, the inline comments name them.'
  fi
fi
[ -n "$scope" ] && review_body="$review_body"$'\n\n'"$(review_mark)"
review+=(-f "body=$review_body")
if ! resp="$(gh api -X POST "repos/$repo/pulls/$pr/reviews" "${review[@]}" 2>&1)"; then
  if rate_limited_error "$resp"; then
    finish failed 1 "echoes: the api rate limit was reached, the review was not posted" "$resp"
  fi
  if is_forbidden "$resp"; then
    readonly_notice
    finish skipped-readonly 0
  fi
  finish failed 1 "echoes: posting the review failed" "$resp"
fi
posted_state="$(printf '%s' "$resp" | jq -r '.state' 2>/dev/null)" || posted_state=''
expected_state='CHANGES_REQUESTED'
[ "$review_event" = "COMMENT" ] && expected_state='COMMENTED'
if [ "$posted_state" != "$expected_state" ]; then
  finish failed 1 "echoes: the review response is unexpected" "$resp"
fi
if [ -n "$failed_log" ]; then
  echo "echoes: the inline comments failed for the following sites"
  printf '%s' "$failed_log"
fi
echo "echoes: posted the review on pull request $pr"
echo "posted" > "$status"
# The rate limit already stopped the posts, the cleanup waits for the next head.
[ "$rate_limited" = "1" ] || cleanup "$(printf '%s' "$resp" | jq -r '.id // empty | tostring' 2>/dev/null)"
