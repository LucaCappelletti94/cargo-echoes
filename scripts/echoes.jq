# Definitions shared by the action's jq programs, which load this module with
# `jq -L "$ECHOES_LIB" 'include "echoes"; ...'`.

# Report text as a markdown code span, the same rules as md_code in lib.sh.
def md_code: (tostring | gsub("[\r\n]"; " ")) as $s
  | ([$s | scan("`+") | length] | max // 0) as $m
  | ("`" * ($m + 1)) as $f
  | (if ($s | test("^`|`$")) then " " + $s + " " else $s end) as $p
  | $f + $p + $f;

# An https url safe to place inside a markdown link, else null.
def safe_url: if type == "string" and test("^https://[A-Za-z0-9.-]+/[^\\s()<>`\\[\\]]*$") then . else null end;

# A site as `path:line`.
def loc: .file + ":" + (.line | tostring);

# The identity of a site of the group with hash $h, its hash, item and path.
def site_key($h): [$h, .item, .file] | join("\t");

# Every site of a report with its group hash: doctest and function groups, context
# groups, and the local site of each reuse group.
def report_sites:
  (.groups[] | .hash as $h | .sites[] | {h: $h, s: .}),
  ((.context_groups // [])[] | .hash as $h | .sites[] | {h: $h, s: .}),
  (((.reuse // {}).groups // [])[] | {h: .hash, s: .local});

# The first site of each of the groups gs, by group hash.
def first_sites(gs): reduce gs[] as $gr ({}; .[$gr.hash] = $gr.sites[0]);

# The line range of a site, `start` or `start-end`, and its blob anchor.
def span: (.line | tostring) + (if .end and .end != .line then "-\(.end)" else "" end);
def anchor: "#L\(.line)" + (if .end and .end != .line then "-L\(.end)" else "" end);

# A site as a list entry, its range linked at commit $sha under $blob when the
# commit is a full sha, plain text otherwise.
def site_md($sha; $blob):
  ((.file + ":" + span) | md_code) as $text
  | "  - "
  + (if ($sha | test("^[0-9a-f]{40}$"))
     then "[\($text)](\($blob)/\($sha)/\(.file | split("/") | map(@uri) | join("/"))\(anchor))"
     else $text end)
  + "  " + (.item | md_code);

# A site as one plain line of an issue form.
def site_line: "\(.file):\(.line)  \(.item)";

# At most twelve lines, with a count of the rest.
def capped: .[:12] + (if length > 12 then ["and \(length - 12) more"] else [] end) | join("\n");

# A dependency reuse candidate as a list entry.
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

# The one-line totals of a report.
def stats: "\(.total) doctests, \(.unique) unique, \(.functions // 0) functions, \(.unique_functions // 0) unique functions, \(.groups | length) duplicated groups";

# The ownership of a repository path, by its nearest directory holding a manifest.
def depth: if . == "." then 0 else (split("/") | length) end;
def under($d): $d == "." or startswith($d + "/");
def owned($dirs): . as $p
  | ([($dirs.owned[] | {d: ., o: true}), ($dirs.foreign[] | {d: ., o: false})]
     | map(select(.d as $d | $p | under($d))) | max_by(.d | depth)) as $n
  | ($n != null and $n.o);

# The finding class a comment posted before the hidden marks opens with.
def legacy_class: if startswith("This context is repeated") then "context"
  elif startswith("This context matches") then "reuse"
  elif (startswith("This doctest") or startswith("This function") or startswith("This method repeats")
        or startswith("This pull request adds")) then "local"
  else null end;
