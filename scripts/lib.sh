# shellcheck shell=bash
# Helpers shared by the action's step scripts, which source this file.

# The directory of the scripts and of the jq module echoes.jq.
ECHOES_LIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# jq with echoes.jq on its module path.
ejq() { jq -L "$ECHOES_LIB" "$@"; }

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

# A gh api failure that names HTTP 403, in its stderr line or its JSON body.
is_forbidden() {
  printf '%s' "$1" | grep -qE 'HTTP 403|"status": "403"'
}

# A 429 is a rate limit, and a 403 naming a rate limit is one too.
rate_limited_error() {
  printf '%s' "$1" | grep -qE 'HTTP 429|"status": "429"' && return 0
  is_forbidden "$1" && printf '%s' "$1" | grep -qiE 'rate limit|x-ratelimit-remaining: 0'
}

# Whole lines within a byte budget, for the review body truncation.
fit_lines() {
  LC_ALL=C awk -v cap="$1" 'BEGIN { b = 0 } { if (b < cap && b + length($0) + 1 <= cap) { print; b += length($0) + 1 } }'
}

uri() { jq -rn --arg s "$1" '$s | @uri'; }

# A repository path inside a url, each segment percent-encoded.
url_path() { jq -rn --arg s "$1" '$s | split("/") | map(@uri) | join("/")'; }

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

# The bare permalink url to lines $3 to $4 of path $2 at commit $1, which GitHub
# expands into a code preview, empty when no commit is known.
blob_url() {
  [ -n "$1" ] || return 0
  local anchor="#L$3"
  [ -n "${4:-}" ] && [ "$4" != "$3" ] && anchor="#L$3-L$4"
  printf '%s/%s/blob/%s/%s%s' "$GITHUB_SERVER_URL" "$GITHUB_REPOSITORY" "$1" "$(url_path "$2")" "$anchor"
}

# The inline text location of a block, `path:start` or `path:start-end`.
loc_text() {
  if [ -n "${3:-}" ] && [ "$3" != "$2" ]; then
    md_code "$1:$2-$3"
  else
    md_code "$1:$2"
  fi
}

# The text wrapped in a markdown link when the url exists.
loc_md() {
  if [ -n "$2" ]; then
    printf '[%s](%s)' "$1" "$2"
  else
    printf '%s' "$1"
  fi
}

# Lines $3 to $4 of path $2 linked at commit $1, plain text without a commit.
blob_link() {
  loc_md "$(loc_text "$2" "$3" "${4:-}")" "$(blob_url "$1" "$2" "$3" "${4:-}")"
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

# Every review thread of pull request $2 of repository $1, one JSON object per line,
# with its first comment's author and body.
list_threads() {
  # shellcheck disable=SC2016 # GraphQL variables, not shell ones
  gh api graphql --paginate \
    -f query='query($owner: String!, $name: String!, $number: Int!, $endCursor: String) {
      repository(owner: $owner, name: $name) { pullRequest(number: $number) {
        reviewThreads(first: 100, after: $endCursor) { pageInfo { hasNextPage endCursor }
          nodes { id isResolved path line comments(first: 1) { nodes { author { login } body } } } } } } }' \
    -f "owner=${1%%/*}" -f "name=${1#*/}" -F "number=$2" \
    --jq '.data.repository.pullRequest.reviewThreads.nodes[] | @json'
}
