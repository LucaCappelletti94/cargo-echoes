//! One-character string patterns of proven text bindings read as `char` patterns.

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, read: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(read));
    assert_eq!(left, right, "{written}\n{read}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(read)),
        "{written}\n{read}"
    );
}

fn separate(left: &str, right: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(left)),
            canonicalize(parse(right)),
            "{left}\n{right}"
        );
    }
}

fn function(params: &str, body: &str) -> String {
    format!("fn f({params}) -> usize {{ {body} }}")
}

#[test]
fn every_pattern_method_reads_its_char_pattern() {
    let calls = [
        ("s.split(\",\").count()", "s.split(',').count()"),
        ("s.rsplit(\",\").count()", "s.rsplit(',').count()"),
        ("s.splitn(2, \",\").count()", "s.splitn(2, ',').count()"),
        (
            "s.split_once(\",\").map_or(0, |(a, _)| a.len())",
            "s.split_once(',').map_or(0, |(a, _)| a.len())",
        ),
        (
            "s.rsplit_once(\",\").map_or(0, |(a, _)| a.len())",
            "s.rsplit_once(',').map_or(0, |(a, _)| a.len())",
        ),
        (
            "usize::from(s.contains(\",\"))",
            "usize::from(s.contains(','))",
        ),
        (
            "usize::from(s.starts_with(\",\"))",
            "usize::from(s.starts_with(','))",
        ),
        (
            "usize::from(s.ends_with(\",\"))",
            "usize::from(s.ends_with(','))",
        ),
        ("s.find(\",\").unwrap_or(0)", "s.find(',').unwrap_or(0)"),
        ("s.rfind(\",\").unwrap_or(0)", "s.rfind(',').unwrap_or(0)"),
        ("s.matches(\",\").count()", "s.matches(',').count()"),
        (
            "s.replace(\",\", \";\").len()",
            "s.replace(',', \";\").len()",
        ),
        (
            "s.replacen(\",\", \";\", 1).len()",
            "s.replacen(',', \";\", 1).len()",
        ),
        (
            "s.strip_prefix(\",\").map_or(0, str::len)",
            "s.strip_prefix(',').map_or(0, str::len)",
        ),
        (
            "s.strip_suffix(\",\").map_or(0, str::len)",
            "s.strip_suffix(',').map_or(0, str::len)",
        ),
        ("s.trim_matches(\",\").len()", "s.trim_matches(',').len()"),
        (
            "s.trim_start_matches(\",\").len()",
            "s.trim_start_matches(',').len()",
        ),
        (
            "s.trim_end_matches(\",\").len()",
            "s.trim_end_matches(',').len()",
        ),
    ];
    for params in ["s: &str", "s: String", "s: &String"] {
        for (written, read) in calls {
            equivalent(&function(params, written), &function(params, read));
        }
    }
}

#[test]
fn escaped_and_multi_byte_characters_read_as_their_char() {
    for (written, read) in [
        ("\"\\n\"", "'\\n'"),
        ("\"'\"", "'\\''"),
        ("\"\\\"\"", "'\"'"),
        ("\"\u{e9}\"", "'\u{e9}'"),
        ("\"\\u{1F600}\"", "'\\u{1F600}'"),
        ("r\"a\"", "'a'"),
    ] {
        equivalent(
            &function("s: &str", &format!("s.split({written}).count()")),
            &function("s: &str", &format!("s.split({read}).count()")),
        );
    }
    equivalent(
        "fn f(v: &'static str) -> usize { let s: &str = v; s.find(\"x\").unwrap_or(0) }",
        "fn f(v: &'static str) -> usize { let s: &str = v; s.find('x').unwrap_or(0) }",
    );
}

#[test]
fn one_character_pushes_read_as_char_pushes() {
    equivalent(
        "fn f(mut s: String) -> String { s.push_str(\"a\"); s }",
        "fn f(mut s: String) -> String { s.push('a'); s }",
    );
    equivalent(
        "fn f(s: &mut String) { s.push_str(\"\\n\") }",
        "fn f(s: &mut String) { s.push('\\n') }",
    );
}

#[test]
fn longer_literals_unproven_receivers_and_replacements_stay_apart() {
    for (left, right) in [
        (
            function("s: &str", "s.split(\"e\\u{301}\").count()"),
            function("s: &str", "s.split('\\u{e9}').count()"),
        ),
        (
            function("s: Text", "s.split(\",\").count()"),
            function("s: Text", "s.split(',').count()"),
        ),
        (
            function("s: &str", "s.replace(',', \";\").len()"),
            function("s: &str", "s.replace(',', ';').len()"),
        ),
        (
            function("s: &str", "s.split(b\",\").count()"),
            function("s: &str", "s.split(',').count()"),
        ),
        (
            function("s: &str", "s.split(\",\"x).count()"),
            function("s: &str", "s.split(',').count()"),
        ),
        (
            "fn f(s: &str) -> String { let mut t = make(); t.push_str(\"a\"); t }".to_owned(),
            "fn f(s: &str) -> String { let mut t = make(); t.push('a'); t }".to_owned(),
        ),
        (
            "trait Parts { fn split(self, pattern: &str) -> usize; }\nfn f(s: String) -> usize { s.split(\",\").count() }".to_owned(),
            "trait Parts { fn split(self, pattern: &str) -> usize; }\nfn f(s: String) -> usize { s.split(',').count() }".to_owned(),
        ),
    ] {
        separate(&left, &right);
    }
}
