//! `char` joins the proven primitives: comparisons, ranges, ASCII predicates and casts.

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

fn function(params: &str, ret: &str, body: &str) -> String {
    format!("fn f({params}) -> {ret} {{ {body} }}")
}

fn rows(params: &str, ret: &str, groups: &[&[&str]]) {
    for group in groups {
        for written in &group[1..] {
            equivalent(
                &function(params, ret, written),
                &function(params, ret, group[0]),
            );
        }
    }
}

#[test]
fn char_comparisons_take_the_integer_orientation() {
    rows(
        "c: char",
        "bool",
        &[
            &["c >= 'a'", "'a' <= c", "!(c < 'a')", "c > '`'"],
            &["c == 'a'", "'a' == c"],
            &["c != 'a'", "'a' != c", "!(c == 'a')"],
            &["c > '\\0'", "c != '\\0'", "c >= '\\u{1}'"],
            &["c == '\\0'", "c < '\\u{1}'", "c <= '\\0'"],
            &["c > '\\u{D7FF}'", "c >= '\\u{E000}'"],
            &["c < '\\u{E000}'", "c <= '\\u{D7FF}'"],
            &["c <= '\\u{10FFFF}'", "'\\u{10FFFF}' >= c"],
            &["true", "c == c"],
            &["c < char::MAX", "c <= '\\u{10FFFE}'"],
            &["c != char::MIN", "c > '\\0'"],
        ],
    );
    equivalent(
        "fn f(c: char) -> bool { let d = c; d >= 'a' }",
        "fn f(c: char) -> bool { let d = c; 'a' <= d }",
    );
}

#[test]
fn char_ranges_read_as_the_comparison_chain() {
    rows(
        "c: char",
        "bool",
        &[
            &[
                "c >= 'a' && c <= 'z'",
                "('a'..='z').contains(&c)",
                "matches!(c, 'a'..='z')",
            ],
            &[
                "c == 'a' || c == 'b'",
                "matches!(c, 'a' | 'b')",
                "matches!(c, 'b' | 'a')",
            ],
            &[
                "c >= '\\0' && c < '\\u{E000}'",
                "('\\0'..'\\u{E000}').contains(&c)",
            ],
        ],
    );
}

#[test]
fn ascii_predicates_read_as_their_ranges() {
    rows(
        "c: char",
        "bool",
        &[
            &["matches!(c, '0'..='9')", "c.is_ascii_digit()"],
            &["matches!(c, 'a'..='z')", "c.is_ascii_lowercase()"],
            &["matches!(c, 'A'..='Z')", "c.is_ascii_uppercase()"],
            &[
                "matches!(c, 'A'..='Z' | 'a'..='z')",
                "c.is_ascii_alphabetic()",
            ],
            &[
                "matches!(c, '0'..='9' | 'A'..='Z' | 'a'..='z')",
                "c.is_ascii_alphanumeric()",
            ],
            &[
                "matches!(c, '0'..='9' | 'A'..='F' | 'a'..='f')",
                "c.is_ascii_hexdigit()",
            ],
            &[
                "matches!(c, '\\t' | '\\n' | '\\x0C' | '\\r' | ' ')",
                "c.is_ascii_whitespace()",
            ],
        ],
    );
    rows(
        "b: u8",
        "bool",
        &[
            &["matches!(b, 48..=57)", "b.is_ascii_digit()"],
            &["matches!(b, 65..=90 | 97..=122)", "b.is_ascii_alphabetic()"],
        ],
    );
}

#[test]
fn lossless_char_casts_read_as_one_spelling() {
    rows("c: char", "u32", &[&["c as u32", "u32::from(c)"]]);
    rows("c: char", "u64", &[&["c as u64", "u64::from(c)"]]);
    rows("b: u8", "char", &[&["b as char", "char::from(b)"]]);
    rows("c: char", "char", &[&["c", "c as char", "char::from(c)"]]);
}

#[test]
fn unlisted_predicates_unproven_receivers_and_calls_stay_apart() {
    for (left, right) in [
        (
            function("c: char", "bool", "c.is_ascii_punctuation()"),
            function("c: char", "bool", "matches!(c, '!'..='/' | ':'..='@' | '['..='`' | '{'..='~')"),
        ),
        (
            function("c: Glyph", "bool", "c.is_ascii_digit()"),
            function("c: Glyph", "bool", "matches!(c, '0'..='9')"),
        ),
        (
            function("c: Glyph", "bool", "c >= 'a'"),
            function("c: Glyph", "bool", "c > '`'"),
        ),
        (
            function("", "bool", "next().is_ascii_digit()"),
            function("", "bool", "next() >= '0' && next() <= '9'"),
        ),
        (
            function("c: char", "u16", "c as u16"),
            function("c: char", "u16", "(c as u32) as u16"),
        ),
        (
            function("c: char", "bool", "c > '\\u{10FFFF}'"),
            function("c: char", "bool", "false"),
        ),
        (
            "trait Digit { fn is_ascii_digit(self) -> bool; }\nfn f(c: char) -> bool { c.is_ascii_digit() }".to_owned(),
            "trait Digit { fn is_ascii_digit(self) -> bool; }\nfn f(c: char) -> bool { matches!(c, '0'..='9') }".to_owned(),
        ),
        (
            function("c: char", "bool", "c == 97char"),
            function("c: char", "bool", "c == 'a'"),
        ),
        (
            function("x: u16", "bool", "x.is_ascii_digit()"),
            function("x: u16", "bool", "matches!(x, 48..=57)"),
        ),
        (
            function("b: bool", "bool", "(0..=1).contains(&b)"),
            function("b: bool", "bool", "b >= 0 && b <= 1"),
        ),
    ] {
        separate(&left, &right);
    }
}
