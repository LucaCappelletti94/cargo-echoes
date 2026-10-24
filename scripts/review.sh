#!/usr/bin/env bash
set -eo pipefail
# The runner drops step summaries over 1 MiB, so a step keeps its
# summary writes within a safe total, the truncated tail gets a note.
sum_append() {
  local text="$1" file="$GITHUB_STEP_SUMMARY"
  local cap=$((900 * 1024)) size=0 room len
  [ -f "$file" ] && size="$(wc -c < "$file")"
  room=$((cap - size))
  len="$(printf '%s' "$text" | wc -c)"
  if [ "$room" -gt 200 ] && [ "$len" -le "$((room - 200))" ]; then
    printf '%s\n' "$text" >> "$file"
  elif [ "$room" -gt 200 ]; then
    head -c "$((room - 200))" <<< "$text" >> "$file"
    printf '\nechoes: the run summary is full, the rest is truncated\n' >> "$file"
  fi
}
# A 429 is a rate limit, and a 403 naming a rate limit is one too.
rate_limited_error() {
  local err="$1"
  if printf '%s' "$err" | grep -qE 'HTTP 429|"status": "429"'; then
    return 0
  fi
  if printf '%s' "$err" | grep -qE 'HTTP 403|"status": "403"'; then
    if printf '%s' "$err" | grep -qiE 'rate limit|x-ratelimit-remaining: 0'; then
      return 0
    fi
  fi
  return 1
}
# Whole lines within a byte budget, for the review body truncation.
fit_lines() {
  LC_ALL=C awk -v cap="$1" 'BEGIN { b = 0 } { if (b < cap && b + length($0) + 1 <= cap) { print; b += length($0) + 1 } }'
}
# No set -e here. Each step checks its own status and writes an outcome file, so a
# failed api call degrades to a skipped review instead of aborting the run.
pr="$PR_NUMBER"
dry="$DRY_RUN"
repo="$GITHUB_REPOSITORY"
report="$RUNNER_TEMP/echoes-report.json"
base_report="$RUNNER_TEMP/echoes-base-report.json"
body="$RUNNER_TEMP/echoes-review.md"
status="$RUNNER_TEMP/echoes-post-status"
inline_file="$RUNNER_TEMP/echoes-inline.tsv"
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
uri() { jq -rn --arg s "$1" '$s | @uri'; }
# Workflow command escaping, a property also ends at `,` or `:`.
escape_data() {
  local s="${1//%/%25}"
  s="${s//$'\r'/%0D}"
  printf '%s' "${s//$'\n'/%0A}"
}
escape_property() {
  local s
  s="$(escape_data "$1")"
  s="${s//:/%3A}"
  printf '%s' "${s//,/%2C}"
}
# Report text as a markdown code span, fenced past its longest backtick run, so
# no mention, link, html or fence in a path or an item renders. A line break
# becomes a space, since an empty line would end the span.
md_code() {
  local s="${1//$'\r'/ }" fence='`'
  s="${s//$'\n'/ }"
  while [[ "$s" == *"$fence"* ]]; do fence="$fence\`"; done
  case "$s" in '`'* | *'`') s=" $s " ;; esac
  printf '%s%s%s' "$fence" "$s" "$fence"
}
# A repository path inside a url, each segment percent-encoded.
url_path() { jq -rn --arg s "$1" '$s | split("/") | map(@uri) | join("/")'; }
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
  reject() {
    printf 'echoes: %s, nothing is posted\n' "$(escape_data "$1")"
    echo "rejected" > "$status"
    exit 1
  }
  stale() {
    printf 'echoes: %s, nothing is posted\n' "$(escape_data "$1")"
    echo "skipped-stale" > "$status"
    exit 0
  }
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
    echo "echoes: cannot read pull request $pr, the review is not posted"
    echo "failed" > "$status"
    exit 1
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
  changed_json="$(jq -r '
    ([.groups[].sites[].file]
     + [(.context_groups // [])[] | .sites[].file]
     + [((.reuse // {}).groups // [])[] | .local.file]) | unique' "$report")"
elif [ -n "$pr" ] && [ -n "$head_sha" ] && [ -n "$changed_files" ]; then
  changed_json="$(printf '%s\n' "$changed_files" | jq -R . | jq -s .)"
else
  changed_json='[]'
fi

# Base site identity, a site exists on the base when the same hash, item and
# path were already there, and the first base copy is the kept original.
if [ "$onlynew" = "1" ]; then
  base_json="$(jq '
    { keys: ([.groups[] | .hash as $h | .sites[] | "\($h)\t\(.item)\t\(.file)"]
             + [(.context_groups // [])[] | .hash as $h | .sites[] | "\($h)\t\(.item)\t\(.file)"]
             + [((.reuse // {}).groups // [])[] | .hash as $h | .local as $l | "\($h)\t\($l.item)\t\($l.file)"]),
      keep: (reduce .groups[] as $gr ({};
        .[$gr.hash] = ($gr.sites[0].file + ":" + ($gr.sites[0].line | tostring)))),
      keep_ctx: (reduce (.context_groups // [])[] as $gr ({};
        .[$gr.hash] = ($gr.sites[0].file + ":" + ($gr.sites[0].line | tostring)))),
      keep_end: (reduce .groups[] as $gr ({}; .[$gr.hash] = ($gr.sites[0].end // null))),
      keep_item: (reduce .groups[] as $gr ({}; .[$gr.hash] = $gr.sites[0].item)) }' "$base_report")"
else
  base_json='{"keys": [], "keep": {}, "keep_ctx": {}, "keep_end": {}, "keep_item": {}}'
fi

# Sites to comment on. In only-new mode a site must be introduced by this
# pull request, a new hash, item or path relative to the base report. A
# site is inline when its file is in the pull request diff.
onlynew_mode=false
[ "$onlynew" = "1" ] && onlynew_mode=true
# One command line argument holds at most 128 KiB, a large pull request's
# file list exceeds it, so jq reads both lists from files.
changed_file="$RUNNER_TEMP/echoes-changed.json"
base_file="$RUNNER_TEMP/echoes-base-keys.json"
printf '%s' "$changed_json" > "$changed_file"
printf '%s' "$base_json" > "$base_file"
jq -r --slurpfile changed "$changed_file" --slurpfile base "$base_file" \
  --argjson onlynew_mode "$onlynew_mode" '
  $changed[0] as $changed
  | $base[0] as $base
  | . as $r
  | [ range(0; ($r.groups | length)) as $gi
      | $r.groups[$gi]
      | ($gi + 1) as $g
      | (.sites | length) as $n
      | .hash as $h
      | (.sites[0].file + ":" + (.sites[0].line | tostring)) as $first
      | (.sites[0].end // null) as $fend
      | (.kind // "doctest") as $kind
      | ([.sites[].self_type] as $t | ($t[0]) as $f | ($t[1:] | map(. != $f) | any)) as $spans
      # A binary older than 0.4.1 sends no remedy, its rule only told types apart.
      | (if $kind == "function" then (.remedy // (if $spans then "generic-or-macro" else "delete" end)) else "delete" end) as $remedy
      | (reduce (.sites[] | (.self_type // "free function")) as $t ([]; if index($t) == null then . + [$t] else . end) | join(", ")) as $selftypes
      | (.sites[0].item) as $firstitem
      | ($base.keep[$h] // "") as $basekeep
      | ($base.keep_item[$h] // "") as $baseitem
      | ([ .sites[] | ([$h, .item, .file] | join("\t")) ]) as $ks
      | [ .sites | to_entries[]
          | (([$h, .value.item, .value.file] | join("\t")) as $k
             | (([ $ks[: .key ][] | select(. == $k) ] | length) + 1) as $idx
             | ([ $base.keys[] | select(. == $k) ] | length) as $bcount
             | { g: $g, c: (.key + 1), n: $n, first: $first, fend: $fend, s: .value,
                 p: .value.file,
                 isNew: ($idx > $bcount),
                 basekeep: $basekeep, bend: ($base.keep_end[$h] // null),
                 kind: $kind, remedy: $remedy, selftypes: $selftypes, firstitem: $firstitem,
                 baseitem: $baseitem }) ] as $rows
      | ($rows | map(select(.isNew)) | length) as $knew
      | $rows[] | . + { knew: $knew }
  ]
  | .[]
  | . as $row
  | select(($changed | index($row.p)) != null)
  | select(if $onlynew_mode then $row.isNew else true end)
  | [$row.p, (.s.line | tostring), .s.item, (.g | tostring), (.c | tostring),
     (.n | tostring), .first, (if .isNew then "1" else "0" end),
     (if .basekeep == "" then "-" else .basekeep end), (.knew | tostring),
     (.s.end // "-" | tostring), (.fend // "-" | tostring), (.bend // "-" | tostring),
     .kind, .remedy, .firstitem,
     (if .baseitem == "" then "-" else .baseitem end), .selftypes,
     (if .s.public then "1" else "0" end)]
  | map(if . == null then "" else tostring | gsub("[\u001f\r\n]"; " ") end) | join("\u001f")' "$report" > "$inline_file"
# The code span and link checks for jq programs, the same rules as md_code.
jq_md='
  def md_code: (tostring | gsub("[\r\n]"; " ")) as $s
    | ([$s | scan("`+") | length] | max // 0) as $m
    | ("`" * ($m + 1)) as $f
    | (if ($s | test("^`|`$")) then " " + $s + " " else $s end) as $p
    | $f + $p + $f;
  def safe_url: if type == "string" and test("^https://[A-Za-z0-9.-]+/[^\\s()<>`\\[\\]]*$") then . else null end;'
# Neutral reuse and context entries, one row per local site that survives
# the same changed-file and only-new filters as the local duplicates.
jq -c --slurpfile changed "$changed_file" --slurpfile base "$base_file" \
  --argjson onlynew_mode "$onlynew_mode" --arg sha "$head_sha" \
  --arg blob "$GITHUB_SERVER_URL/$repo/blob" "$jq_md"'
  def spath: .file;
  def span: (.line | tostring) + (if .end and .end != .line then "-\(.end)" else "" end);
  def anchor: "#L\(.line)" + (if .end and .end != .line then "-L\(.end)" else "" end);
  def site_md:
    ((spath + ":" + span) | md_code) as $text
    | "  - "
    + (if ($sha | test("^[0-9a-f]{40}$"))
       then "[\($text)](\($blob)/\($sha)/\(spath | split("/") | map(@uri) | join("/"))\(anchor))"
       else $text end)
    + "  " + (.item | md_code);
  def site_line: "\(spath):\(.line)  \(.item)";
  def capped: .[:12] + (if length > 12 then ["and \(length - 12) more"] else [] end) | join("\n");
  def cand_md:
    "  - " + ("\(.package) \(.version)" | md_code)
    + (if (.depth | type) == "number" and .depth > 0 then " (depth \(.depth | floor))" else "" end)
    + (if ((.conditions // []) | length) > 0 then " (needs " + (.conditions | map(md_code) | join(", ")) + ")" else "" end)
    + (if .action == "reuse-api" then ": the public item " + ((.api_path // .site.item) | md_code) + " can be reused"
       elif .action == "upgrade-api" then ": the public item " + ((.api_path // .site.item) | md_code) + " is available in a newer published release"
       elif .action == "request-extraction" then ": the internal item " + (.site.item | md_code) + " can be requested for extraction"
       elif .action == "unresolved" then ": source analysis does not establish accessibility"
       else ": the reuse path is unresolved" end)
    + ((.source_url | safe_url) as $u
       | if $u == null then ""
         elif .api_path and .api_path != .site.item then " [source (" + (.site.item | md_code) + ")](" + $u + ")"
         else " [source](" + $u + ")" end)
    + (if .existing_issue then
         ((.existing_issue.url | safe_url) as $u
          | ((.existing_issue.state | tostring) | if test("^[a-z]+$") then . else "unknown" end) as $st
          | if $u then " [existing request \($st)](\($u))" else "" end)
       elif (.issue_url | safe_url) then " [request extraction](\(.issue_url))"
       elif .action == "request-extraction" then " (the upstream repository is not declared, so no issue link is offered)"
       else "" end)
    + (if .lookup_warning then "\n    lookup: " + (.lookup_warning | md_code) else "" end);
  $changed[0] as $changed
  | $base[0] as $base
  | [ ( (.context_groups // [])
        | .[] as $gr
        | ($base.keep_ctx[$gr.hash] // "") as $basekeep
        | ($gr.sites | length) as $n
        | range(0; $n) as $i
            | (if $i == 0 and (($basekeep == "") or ($onlynew_mode | not)) then empty else $gr.sites[$i] end) as $s
            | ($gr.hash + "\t" + $s.item + "\t" + ($s | spath)) as $k
            | { path: ($s | spath), line: $s.line, end: $s.end, item: $s.item,
                kind: "context", hash: $gr.hash, label: "context group \($gr.hash[:8])",
                isNew: (([ $base.keys[] | select(. == $k) ] | length) == 0),
                others: ([ $gr.sites | to_entries[] | select(.key != $i) | .value | site_md ]),
                sites: ([ $gr.sites[] | site_line ] | capped) }
      )
      , ( ((.reuse // {}).groups // [])
          | .[] as $gr
          | $gr.local as $l
          | ($gr.hash + "\t" + $l.item + "\t" + ($l | spath)) as $k
          | { path: ($l | spath), line: $l.line, end: $l.end, item: $l.item,
              kind: "reuse", hash: $gr.hash, label: "reuse group \($gr.hash[:8])",
              isNew: (([ $base.keys[] | select(. == $k) ] | length) == 0),
              candidates_md: ([ $gr.candidates[] | cand_md ] | join("\n")),
              sites: ([ ($l | site_line) ] + [ $gr.candidates[] | "\(.package) \(.version)  \(.site.item)" ] | capped) } )
    ]
  | map(. as $e | select(($changed | index($e.path)) != null))
  | map(select(if $onlynew_mode then .isNew else true end))
  | .[]
' "$report" > "$reuse_tmp"

# The inline posts, live mode only. The first post is the permission probe.
# Each comment links to a prefilled wrong-review issue naming its label and sites.
report_link() {
  printf '[👎 Report a wrong review](https://github.com/LucaCappelletti94/cargo-echoes/issues/new?template=wrong-review.yml&title=%s&version=%s&review=%s&group=%s)' \
    "$(uri "Wrong review on $repo#$pr, $1")" \
    "$(uri "$version")" \
    "$(uri "$GITHUB_SERVER_URL/$repo/pull/$pr")" \
    "$(uri "$2")"
}
group_sites() {
  jq -r --argjson g "$1" '
    .groups[$g - 1].sites as $s
    | [$s[:12][] | "\(.file):\(.line)  \(.item)"]
      + (if ($s | length) > 12 then ["and \(($s | length) - 12) more"] else [] end)
    | join("\n")' "$report"
}
support_link='[❤️ Support echoes](https://github.com/sponsors/LucaCappelletti94)'
# The hidden marks that tie a comment to its workspace and finding, and a review to
# its workspace, so a later head finds what this run posted.
thread_mark() { printf '<!-- echoes:v1 scope=%s key=%s -->' "$scope_uri" "$1"; }
review_mark() { printf '<!-- echoes:v1 scope=%s review -->' "$scope_uri"; }
finding_key() { uri "$1:$2:$3:$4"; }
# The ownership of a repository path, by its nearest directory holding a manifest,
# and the finding class a comment posted before the marks opens with.
jq_own='
  def depth: if . == "." then 0 else (split("/") | length) end;
  def under($d): $d == "." or startswith($d + "/");
  def owned($dirs): . as $p
    | ([($dirs.owned[] | {d: ., o: true}), ($dirs.foreign[] | {d: ., o: false})]
       | map(select(.d as $d | $p | under($d))) | max_by(.d | depth)) as $n
    | ($n != null and $n.o);
  def legacy_class: if startswith("This context is repeated") then "context"
    elif startswith("This context matches") then "reuse"
    elif (startswith("This doctest") or startswith("This function") or startswith("This method repeats")
          or startswith("This pull request adds")) then "local"
    else null end;'
# Every review thread of the pull request, one JSON object per line.
list_threads() {
  gh api graphql --paginate \
    -f query='query($owner: String!, $name: String!, $number: Int!, $endCursor: String) {
      repository(owner: $owner, name: $name) { pullRequest(number: $number) {
        reviewThreads(first: 100, after: $endCursor) { pageInfo { hasNextPage endCursor }
          nodes { id isResolved path line comments(first: 1) { nodes { author { login } body } } } } } } }' \
    -f "owner=${repo%%/*}" -f "name=${repo#*/}" -F "number=$pr" \
    --jq '.data.repository.pullRequest.reviewThreads.nodes[] | @json'
}
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
      if printf '%s' "$err" | grep -qE 'HTTP 403|"status": "403"'; then
        refused=1
        break
      fi
      printf 'echoes: dismissing review %s failed\n' "$id"
      printf '%s\n' "$err" | sed 's/^/echoes: /'
      continue
    fi
    n=$((n + 1))
  done < <(printf '%s\n' "$reviews" | jq -r --slurpfile dirs "$dirs_file" --arg mark "$(review_mark)" --arg keep "$keep" "$jq_own"'
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
# A permalink to lines $3 to $4 of path $2 at commit $1, GitHub shows
# and highlights those lines. Without a sha it falls back to plain text.
blob_link() {
  local anchor label
  if [ -n "${4:-}" ] && [ "$4" != "$3" ]; then
    anchor="#L$3-L$4"
    label="$3-$4"
  else
    anchor="#L$3"
    label="$3"
  fi
  if [ -n "$1" ]; then
    printf '[%s](%s/%s/blob/%s/%s%s)' "$(md_code "$2:$label")" "$GITHUB_SERVER_URL" "$repo" "$1" "$(url_path "$2")" "$anchor"
  else
    md_code "$2:$label"
  fi
}
# The bare permalink url GitHub expands into a code preview of the
# cited lines, empty when no commit is known.
blob_url() {
  [ -n "$1" ] || return 0
  if [ -n "${4:-}" ] && [ "$4" != "$3" ]; then
    printf '%s/%s/blob/%s/%s#L%s-L%s' "$GITHUB_SERVER_URL" "$repo" "$1" "$(url_path "$2")" "$3" "$4"
  else
    printf '%s/%s/blob/%s/%s#L%s' "$GITHUB_SERVER_URL" "$repo" "$1" "$(url_path "$2")" "$3"
  fi
}
# The inline text location of a block, `path:start` or `path:start-end`.
loc_text() {
  if [ -n "${3:-}" ] && [ "$3" != "$2" ]; then
    md_code "$1:$2-$3"
  else
    md_code "$1:$2"
  fi
}
# The permalink wrapped in a markdown link when it exists.
loc_md() {
  if [ -n "$2" ]; then
    printf '[%s](%s)' "$1" "$2"
  else
    printf '%s' "$1"
  fi
}
# The bare url GitHub previews as code, collapsed in a details block
# when the block spans more than ten lines. Report numbers stay out of shell
# arithmetic, which would run a command substitution inside them.
preview_of() {
  local n="${3:-$2}" label="${4:-the duplicated doctest}"
  if [ "$(awk -v s="$2" -v e="$n" 'BEGIN { print (e - s + 1 > 10) }')" = 1 ]; then
    printf '<details>\n<summary>Show %s</summary>\n\n%s\n\n</details>' "$label" "$1"
  else
    printf '%s' "$1"
  fi
}
rendered=''
findings=''
findings_span=0
# One pass builds every comment, live posts them, the dry run renders them.
# The end line of each block comes from the report, `-` when the crate has
# none, a block spanning an include or an escaped literal, or an older binary.
# Fields are split on the unit separator, which read never merges the way it
# merges runs of tabs, so an empty field cannot shift the ones after it.
declare -A seen_first=() seen_first_end=()
while IFS=$'\x1f' read -r p line item g c n first new basekeep knew end fend bend kind remedy firstitem baseitem selftypes public; do
  [ -n "$p" ] || continue
  # A public copy that already shipped is called or deprecated, deleting it breaks callers.
  if [ "$remedy" = "delete" ] && [ "$kind" = "function" ] && [ "$public" = "1" ] && [ "$onlynew" != "1" ]; then
    remedy=public
  fi
  site="$p:$line"
  [ "$basekeep" = "-" ] && basekeep=''
  [ "$end" = "-" ] && end=''
  [ "$fend" = "-" ] && fend=''
  [ "$bend" = "-" ] && bend=''
  [ "$baseitem" = "-" ] && baseitem=''
  kept_item="$firstitem"
  [ -n "$baseitem" ] && kept_item="$baseitem"
  if [ "$new" = "1" ] && [ -z "${seen_first[$g]:-}" ]; then
    seen_first[$g]="$site"
    seen_first_end[$g]="$end"
  fi
  if [ "$onlynew" = "1" ]; then
    if [ -n "$basekeep" ] || [ "$site" != "${seen_first[$g]}" ]; then removable=1; else removable=0; fi
  elif [ "$c" != "1" ]; then
    removable=1
  else
    removable=0
  fi
  keepat="$first"
  keepat_end="$fend"
  if [ "$new" = "1" ]; then
    keepat="${seen_first[$g]:-$first}"
    keepat_end="${seen_first_end[$g]:-$fend}"
  fi
  # Comments only land on copies the author should act on, a kept
  # copy stays silent.
  [ "$removable" = "1" ] || continue
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
  close_line="$end"
  if [ -z "$close_line" ]; then
    out_of_range="$out_of_range- group $g  $(blob_link "$head_sha" "$p" "$line")  $(md_code "$item")"$'\n'
    oor_local=1
    continue
  fi
  # The suggestion deletes the block plus the empty line right after
  # it, which would become a stray once the copy is gone.
  delete_end="$close_line"
  if [ "$remedy" = "delete" ]; then
    if grep -Fxq -- "$p:$close_line" "$blank_file" 2>/dev/null; then
      delete_end="$(awk -v l="$close_line" 'BEGIN { print l + 1 }')"
    fi
  fi
  url=''
  loc=''
  pv_s=''
  pv_e=''
  if [ "$onlynew" = "1" ] && [ -n "$basekeep" ]; then
    bp="${basekeep%:*}"
    bl="${basekeep##*:}"
    ce="$bend"
    url="$(blob_url "$base_sha" "$bp" "$bl" "$ce")"
    pv_s="$bl"
    pv_e="$ce"
    loc="$(loc_md "$(loc_text "$bp" "$bl" "$ce")" "$url")"
  elif [ "$onlynew" = "1" ]; then
    kp="${keepat%:*}"
    kl="${keepat##*:}"
    ce="$keepat_end"
    url="$(blob_url "$head_sha" "$kp" "$kl" "$ce")"
    pv_s="$kl"
    pv_e="$ce"
    loc="$(loc_md "$(loc_text "$kp" "$kl" "$ce")" "$url")"
  else
    fp="${first%:*}"
    fl="${first##*:}"
    ce="$fend"
    url="$(blob_url "$head_sha" "$fp" "$fl" "$ce")"
    pv_s="$fl"
    pv_e="$ce"
    loc="$(loc_md "$(loc_text "$fp" "$fl" "$ce")" "$url")"
  fi
  echoes_link='[echoes](https://github.com/LucaCappelletti94/cargo-echoes)'
  if [ "$kind" = "function" ]; then
    if [ "$remedy" != "delete" ]; then
      preview='the repeated function'
    else
      preview='the duplicated function'
    fi
  else
    preview='the duplicated doctest'
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
    note="This doctest was added by this pull request and duplicates a copy that already exists at $loc, [echoes](https://github.com/LucaCappelletti94/cargo-echoes) suggests removing the added copy."
  elif [ "$onlynew" = "1" ]; then
    note="This pull request adds $knew copies of the same doctest, [echoes](https://github.com/LucaCappelletti94/cargo-echoes) suggests keeping the first added copy at $loc and removing this one."
  else
    note="This doctest appears $n times in this scan, this is copy $c. [echoes](https://github.com/LucaCappelletti94/cargo-echoes) suggests keeping the first copy at $loc and removing this one."
  fi
  comment="$note"
  [ -n "$url" ] && comment="$comment"$'\n\n'"$(preview_of "$url" "$pv_s" "$pv_e" "$preview")"
  if [ "$remedy" = "delete" ]; then
    comment="$comment"$'\n\n```suggestion\n```'
  fi
  comment="$comment"$'\n\n'"$(report_link "group $g" "$(group_sites "$g")") $support_link"
  if [ "$dry" = "true" ]; then
    rendered+="$comment"$'\n\n'
    continue
  fi
  # One row per comment, so a later job can post it without the
  # source tree it was computed from.
  key="$(finding_key "$kind" "$(jq -r --argjson g "$g" '.groups[$g - 1].hash' "$report")" "$p" "$item")"
  [ -n "$scope" ] && comment="$comment"$'\n\n'"$(thread_mark "$key")"
  jq -cn --arg path "$p" --arg start "$line" --arg stop "$delete_end" \
    --arg group "$g" --arg item "$item" --arg close "$close_line" --arg key "$key" --arg body "$comment" \
    '{path: $path, start_line: ($start | tonumber), line: ($stop | tonumber),
      group: $group, item: $item, close_line: ($close | tonumber), key: $key, body: $body}' \
    >> "$comments"
done < "$inline_file"
# The neutral reuse and context entries, the local comment minus the
# removal suggestion, so approximate and upstream matches stay advice.
reuse_rendered=''
if [ -s "$reuse_tmp" ]; then
  reuse_built="$RUNNER_TEMP/echoes-reuse-built.ndjson"
  : > "$reuse_built"
  while IFS= read -r row; do
    [ -n "$row" ] || continue
    p="$(printf '%s' "$row" | jq -r '.path')"
    line="$(printf '%s' "$row" | jq -r '.line')"
    end="$(printf '%s' "$row" | jq -r '.end')"
    item="$(printf '%s' "$row" | jq -r '.item')"
    kind="$(printf '%s' "$row" | jq -r '.kind')"
    others="$(printf '%s' "$row" | jq -r '.others // [] | join("\n")')"
    candidates_md="$(printf '%s' "$row" | jq -r '.candidates_md // empty')"
    if [ "$kind" = "context" ]; then
      note="This context is repeated in this scan. A shared helper can carry it once. The other copies are at"
      [ -n "$others" ] && note="$note"$'\n'"$others"
    else
      note="This context matches an upstream dependency context. The accessible APIs are listed first"
      note="$note"$'\n'"$candidates_md"
    fi
    note="$note"$'\n\n'"Type compatibility and behavior equivalence need human review."
    r_url="$(blob_url "$head_sha" "$p" "$line" "$end")"
    comment="$note"
    [ -n "$r_url" ] && comment="$comment"$'\n\n'"$(preview_of "$r_url" "$line" "$end" "the local context")"
    comment="$comment"$'\n\n'"$(report_link "$(printf '%s' "$row" | jq -r '.label')" "$(printf '%s' "$row" | jq -r '.sites')") $support_link"
    if [ "$dry" = "true" ]; then
      reuse_rendered+="$comment"$'\n\n'
      continue
    fi
    key="$(finding_key "$kind" "$(printf '%s' "$row" | jq -r '.hash')" "$p" "$item")"
    [ -n "$scope" ] && comment="$comment"$'\n\n'"$(thread_mark "$key")"
    jq -cn --arg path "$p" --arg start "$line" --arg stop "$end" \
      --arg item "$item" --arg kind "$kind" --arg key "$key" --arg body "$comment" \
      '{path: $path, start_line: ($start | tonumber), line: ($stop | tonumber),
        item: $item, kind: $kind, key: $key, body: $body}' >> "$reuse_built"
  done < "$reuse_tmp"
  mv "$reuse_built" "$reuse_tmp"
fi

if [ "$dry" = "true" ]; then
  stats="$(jq -r '"\(.total) doctests, \(.unique) unique, \(.functions // 0) functions, \(.unique_functions // 0) unique functions, \(.groups | length) duplicated groups"' "$report")"
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
  echo "dry-run" > "$status"
  exit 0
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
  if list_threads > "$threads_raw" 2>/dev/null; then
    threads_ok=1
    jq -s --slurpfile dirs "$dirs_file" --arg scope "$scope_uri" "$jq_own"'
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
  jq -rs "$jq_md"'map("- " + ((.path + ":" + (.start_line | tostring)) | md_code) + "  " + (.item | md_code)
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
    echo "echoes: every finding of this pull request already has a review thread, nothing new is posted"
    echo "skipped-existing" > "$status"
  elif [ "$onlynew" = "1" ]; then
    echo "echoes: no duplicate was introduced by this pull request, the review is not posted"
    echo "skipped-existing" > "$status"
  else
    echo "echoes: no duplicate is in the pull request diff, the review is not posted"
    echo "skipped-nodiff" > "$status"
  fi
  [ "$mode" = "prepare" ] || cleanup
  exit 0
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
  echo "echoes: the review is prepared in $payload for a later job"
  echo "prepared" > "$status"
  exit 0
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
  local row="$1" p line stop err item g
  p="$(printf '%s' "$row" | jq -r '.path')"
  line="$(printf '%s' "$row" | jq -r '.start_line')"
  stop="$(printf '%s' "$row" | jq -r '.line')"
  item="$(printf '%s' "$row" | jq -r '.item')"
  g="$(printf '%s' "$row" | jq -r '.group // .kind')"
  if ! err="$(printf '%s' "$row" | jq -r '.body' | gh api "repos/$repo/pulls/$pr/comments" \
    -f "commit_id=$head_sha" -f "path=$p" \
    -F "start_line=$line" -F "line=$stop" \
    -f start_side=RIGHT -f side=RIGHT -F 'body=@-' 2>&1)"; then
    if rate_limited_error "$err"; then
      rate_limited=1
      return 1
    fi
    if printf '%s' "$err" | grep -qE 'HTTP 403|"status": "403"'; then
      readonly=1
      return 1
    fi
    printf 'echoes: %s:%s %s\n' "$(escape_data "$p")" "$line" "$(escape_data "$item")"
    printf '%s\n' "$err" | sed 's/^/echoes: /'
    if printf '%s' "$err" | grep -qE 'HTTP 422|"status": "422"'; then
      out_of_range="$out_of_range- group $g  $(blob_link "$head_sha" "$p" "$line" "$(printf '%s' "$row" | jq -r '.close_line // .line')")  $(md_code "$item")"$'\n'
    else
      failed_sites="$failed_sites- group $g  $(md_code "$p:$line")  $(md_code "$item")"$'\n'
      failed_log="$failed_log  group $g  $(escape_data "$p"):$line  $(escape_data "$item")"$'\n'
    fi
  fi
  return 0
}
while IFS= read -r row; do
  [ -n "$row" ] || continue
  post_one "$row" || break
  posted_local=$((posted_local + 1))
done < "$comments"
if [ "$readonly" != "1" ] && [ "$rate_limited" != "1" ]; then
  while IFS= read -r row; do
    [ -n "$row" ] || continue
    post_one "$row" || break
    posted_reuse=$((posted_reuse + 1))
  done < "$reuse_tmp"
fi
if [ "$readonly" = "1" ]; then
  readonly_notice
  echo "skipped-readonly" > "$status"
  exit 0
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
review_body=''
if [ -n "$out_of_range" ]; then
  review_body="$(printf 'These duplicates are not on lines GitHub shows in this pull request diff, the inline comment cannot attach there. The links point at the copies on the head commit.\n\n%s' "$out_of_range")"
fi
if [ -n "$failed_sites" ]; then
  review_body="$review_body$(printf '\n\nDuplicates echoes could not comment inline\n\n%s' "$failed_sites")"
fi
if [ "$omitted" -gt 0 ]; then
  review_body="$review_body$(printf '\n\n%s inline copies were not posted, they are listed in the run summary %s' "$omitted" "$run_url")"
fi
if [ "$rate_omitted" -gt 0 ]; then
  review_body="$review_body$(printf '\n\n%s inline copies were not posted because the api rate limit was reached, they are listed in the run summary %s' "$rate_omitted" "$run_url")"
fi
# The api rejects a review body over 65,536 characters, so the lists
# are truncated with a pointer to the run summary.
if [ "${#review_body}" -gt 65000 ]; then
  budget=$((65000 / 2 - 400))
  oor_part=''
  if [ -n "$out_of_range" ]; then
    oor_kept="$(printf '%s' "$out_of_range" | fit_lines "$budget")"
    oor_more=$(($(printf '%s' "$out_of_range" | wc -l) - $(printf '%s' "$oor_kept" | wc -l)))
    oor_part='These duplicates are not on lines GitHub shows in this pull request diff, the inline comment cannot attach there. The links point at the copies on the head commit.'
    oor_part="$oor_part"$'\n\n'"$oor_kept"
    [ "$oor_more" -gt 0 ] && oor_part="$oor_part"$'\n'"and $oor_more more, they are in the run summary $run_url"
  fi
  failed_part=''
  if [ -n "$failed_sites" ]; then
    failed_kept="$(printf '%s' "$failed_sites" | fit_lines "$budget")"
    failed_more=$(($(printf '%s' "$failed_sites" | wc -l) - $(printf '%s' "$failed_kept" | wc -l)))
    failed_part="$(printf '\n\nDuplicates echoes could not comment inline\n\n%s' "$failed_kept")"
    [ "$failed_more" -gt 0 ] && failed_part="$failed_part"$'\n'"and $failed_more more, they are in the run summary $run_url"
  fi
  review_body="$oor_part$failed_part"
  [ "$omitted" -gt 0 ] && review_body="$review_body$(printf '\n\n%s inline copies were not posted, they are listed in the run summary %s' "$omitted" "$run_url")"
  [ "$rate_omitted" -gt 0 ] && review_body="$review_body$(printf '\n\n%s inline copies were not posted because the api rate limit was reached, they are listed in the run summary %s' "$rate_omitted" "$run_url")"
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
    echo "echoes: the api rate limit was reached, the review was not posted"
    printf '%s\n' "$resp"
    echo "failed" > "$status"
    exit 1
  fi
  if printf '%s' "$resp" | grep -qE 'HTTP 403|"status": "403"'; then
    readonly_notice
    echo "skipped-readonly" > "$status"
    exit 0
  fi
  echo "echoes: posting the review failed"
  printf '%s\n' "$resp"
  echo "failed" > "$status"
  exit 1
fi
posted_state="$(printf '%s' "$resp" | jq -r '.state' 2>/dev/null)" || posted_state=''
expected_state='CHANGES_REQUESTED'
[ "$review_event" = "COMMENT" ] && expected_state='COMMENTED'
if [ "$posted_state" != "$expected_state" ]; then
  echo "echoes: the review response is unexpected"
  printf '%s\n' "$resp"
  echo "failed" > "$status"
  exit 1
fi
if [ -n "$failed_log" ]; then
  echo "echoes: the inline comments failed for the following sites"
  printf '%s' "$failed_log"
fi
echo "echoes: posted the review on pull request $pr"
echo "posted" > "$status"
# The rate limit already stopped the posts, the cleanup waits for the next head.
[ "$rate_limited" = "1" ] || cleanup "$(printf '%s' "$resp" | jq -r '.id // empty | tostring' 2>/dev/null)"
