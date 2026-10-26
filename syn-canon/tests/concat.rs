//! Built-in `concat!` of string literals over resolved macro names.

#[path = "support/view.rs"]
mod view;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(concatenated: &str, literal: &str) {
    let left = syn_canon::canonicalize(parse(concatenated));
    let right = syn_canon::canonicalize(parse(literal));
    assert_eq!(left, right, "{concatenated}\n{literal}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens());
    assert_eq!(left.body_units(), right.body_units());
    assert_ne!(
        syn_canon::canonicalize_failing(parse(concatenated)),
        syn_canon::canonicalize_failing(parse(literal)),
        "{concatenated}\n{literal}",
    );
}

fn separate(concatenated: &str, literal: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(concatenated)),
            canonicalize(parse(literal)),
            "{concatenated}\n{literal}"
        );
    }
}

fn function(body: &str) -> String {
    format!("fn f() -> &'static str {{ {body} }}")
}

#[test]
fn string_literal_arguments_fold_to_their_joined_value() {
    for (concatenated, literal) in [
        (r#"concat!("a", "b")"#, r#""ab""#),
        (r#"concat!("a", "b",)"#, r#""ab""#),
        (r#"concat!["a", "b"]"#, r#""ab""#),
        (r#"concat!("a")"#, r#""a""#),
        ("concat!()", r#""""#),
        (r#"concat!("", "a", "")"#, r#""a""#),
        (r#"concat!("ü", "日本", "🦀")"#, r#""ü日本🦀""#),
        (r#"std::concat!("a", "b")"#, r#""ab""#),
        (r#"core::concat!("a", "b")"#, r#""ab""#),
    ] {
        equivalent(&function(concatenated), &function(literal));
    }
}

#[test]
fn escapes_and_raw_strings_compare_by_their_value() {
    for (concatenated, literal) in [
        (r#"concat!("a\n", "\u{e9}", "\x41")"#, r#""a\néA""#),
        (r#"concat!("tab\t", "quote\"")"#, r#""tab\tquote\"""#),
        (r###"concat!(r"a\n", r##"b"c"##)"###, r#""a\\nb\"c""#),
        (
            r#"concat!("line\
              continued")"#,
            r#""linecontinued""#,
        ),
    ] {
        equivalent(&function(concatenated), &function(literal));
    }
}

#[test]
fn every_value_position_folds() {
    for (concatenated, literal) in [
        (
            r#"fn f() -> &'static str { let s = concat!("a", "b"); s }"#,
            r#"fn f() -> &'static str { let s = "ab"; s }"#,
        ),
        (
            r#"const S: &str = concat!("a", "b");"#,
            r#"const S: &str = "ab";"#,
        ),
        (
            r#"fn f() -> [&'static str; 2] { [concat!("a"), concat!("b", "c")] }"#,
            r#"fn f() -> [&'static str; 2] { ["a", "bc"] }"#,
        ),
        (
            r#"fn f() { take(concat!("a", "b")); }"#,
            r#"fn f() { take("ab"); }"#,
        ),
        (
            r#"static S: &str = concat!("a", "b");"#,
            r#"static S: &str = "ab";"#,
        ),
        (
            r#"fn f() -> &'static str { let g = || concat!("a", "b"); g() }"#,
            r#"fn f() -> &'static str { let g = || "ab"; g() }"#,
        ),
        (
            r#"fn f(x: u8) -> &'static str { match x { 0 => concat!("a", "b"), _ => "c" } }"#,
            r#"fn f(x: u8) -> &'static str { match x { 0 => "ab", _ => "c" } }"#,
        ),
        (
            r#"fn f() -> &'static str { loop { break concat!("a", "b"); } }"#,
            r#"fn f() -> &'static str { loop { break "ab"; } }"#,
        ),
        (
            r#"fn f(x: u32) -> (u32, &'static str) { let s = concat!("a", "b"); let y = x ^ 1u32; (y, s) }"#,
            r#"fn f(x: u32) -> (u32, &'static str) { let s = "ab"; let y = x ^ 1u32; (y, s) }"#,
        ),
        (
            r#"struct P { a: u32, b: &'static str } fn f(x: u32) -> P { P { b: concat!("x", "y"), a: x } }"#,
            r#"fn f(x: u32) -> P { P { b: "xy", a: x } } struct P { a: u32, b: &'static str }"#,
        ),
    ] {
        equivalent(concatenated, literal);
    }
}

#[test]
fn shadowed_imported_and_custom_macros_keep_their_input() {
    let rules = r#"macro_rules! concat { ($($t:tt)*) => { "x" } }"#;
    for (scope, call) in [
        (rules, r#"concat!("a", "b")"#),
        ("use other::concat;", r#"concat!("a", "b")"#),
        ("use other::*;", r#"concat!("a", "b")"#),
        ("", r#"my::concat!("a", "b")"#),
        ("use other as std;", r#"std::concat!("a", "b")"#),
    ] {
        separate(
            &format!("{scope} {}", function(call)),
            &format!("{scope} {}", function(r#""ab""#)),
        );
    }
}

#[test]
fn unsupported_arguments_keep_the_call() {
    for (concatenated, literal) in [
        (r#"concat!("a", 1)"#, r#""a1""#),
        (r#"concat!("a", 'b')"#, r#""ab""#),
        (r#"concat!("a", true)"#, r#""atrue""#),
        (r#"concat!("a", -1)"#, r#""a-1""#),
        (r#"concat!(b"a")"#, r#""a""#),
        (r#"concat!(concat!("a"), "b")"#, r#""ab""#),
        (r#"concat!("a", S)"#, r#""ab""#),
        (r#"concat!("a"; 2)"#, r#""aa""#),
        (r#"concat!("a" "b")"#, r#""ab""#),
        (r#"concat!("a", ("b"))"#, r#""ab""#),
        (r#"concat!("a"x)"#, r#""a""#),
    ] {
        separate(&function(concatenated), &function(literal));
    }
}

#[test]
fn observed_syntax_keeps_the_call() {
    for (concatenated, literal) in [
        (
            r#"#[observe] fn f() -> &'static str { concat!("a", "b") }"#,
            r#"#[observe] fn f() -> &'static str { "ab" }"#,
        ),
        (
            r#"fn f() -> &'static str { #[observe] concat!("a", "b") }"#,
            r#"fn f() -> &'static str { #[observe] "ab" }"#,
        ),
        (
            r#"fn f() { unknown!(concat!("a", "b")); }"#,
            r#"fn f() { unknown!("ab"); }"#,
        ),
        (
            r#"fn f() { stringify!(concat!("a", "b")); }"#,
            r#"fn f() { stringify!("ab"); }"#,
        ),
        (
            r#"fn f() -> String { format!("{}", concat!("a", "b")) }"#,
            r#"fn f() -> String { format!("{}", "ab") }"#,
        ),
    ] {
        separate(concatenated, literal);
    }
}

#[test]
fn borrowed_views_share_the_fold_without_expanding_lexical_proofs() {
    let file =
        parse(r#"fn a() -> &'static str { concat!("a", "b") } fn b() -> &'static str { "ab" }"#);
    assert_eq!(view::view_of(&file, "a"), view::view_of(&file, "b"));
    let file = parse(
        r#"use other::concat; fn a() -> &'static str { concat!("a", "b") } fn b() -> &'static str { "ab" }"#,
    );
    assert_ne!(view::view_of(&file, "a"), view::view_of(&file, "b"));
}
