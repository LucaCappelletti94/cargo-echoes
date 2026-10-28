//! Booleans read as integers and integer `min` and `max` share one form.

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

fn separate(written: &str, read: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(read)),
            "{written}\n{read}"
        );
    }
}

fn pair(body: &str) -> String {
    format!("fn f(x: u32, y: u32, p: bool) -> u32 {{ {body} }}")
}

#[test]
fn booleans_read_as_integers() {
    for (written, read) in [
        ("if p { 1u32 } else { 0u32 }", "p as u32"),
        ("if p { 1u32 } else { 0u32 }", "u32::from(p)"),
        ("if p { 0u32 } else { 1u32 }", "!p as u32"),
        ("if x < y { 1u32 } else { 0u32 }", "(x < y) as u32"),
        (
            "let v: u32 = if p { 1 } else { 0 }; v",
            "let v: u32 = p as u32; v",
        ),
    ] {
        equivalent(&pair(written), &pair(read));
    }
    equivalent(
        "fn f(p: bool) -> u8 { let v: u8 = if p { 1 } else { 0 }; v }",
        "fn f(p: bool) -> u8 { let v: u8 = p as u8; v }",
    );
}

#[test]
fn max_and_min_share_one_form() {
    for (written, read) in [
        ("x.max(y)", "y.max(x)"),
        ("std::cmp::max(x, y)", "x.max(y)"),
        ("core::cmp::max(y, x)", "x.max(y)"),
        ("if x > y { x } else { y }", "x.max(y)"),
        ("if x >= y { x } else { y }", "y.max(x)"),
        ("if y < x { x } else { y }", "x.max(y)"),
        ("x.min(y)", "y.min(x)"),
        ("std::cmp::min(x, y)", "x.min(y)"),
        ("if x < y { x } else { y }", "x.min(y)"),
        ("if x <= y { x } else { y }", "y.min(x)"),
        ("if x > y { y } else { x }", "x.min(y)"),
    ] {
        equivalent(&pair(written), &pair(read));
    }
    equivalent(
        "use core as c; fn f(x: u32, y: u32) -> u32 { c::cmp::max(x, y) }",
        "use core as c; fn f(x: u32, y: u32) -> u32 { x.max(y) }",
    );
    equivalent(
        "fn f(p: bool) -> u32 { let v: u32 = if p { 1 } else { return 0; }; v }",
        "fn f(p: bool) -> u32 { let v: u32 = if p { 1u32 } else { return 0; }; v }",
    );
    equivalent(&pair("if true { 1u32 } else { 0u32 }"), &pair("1u32"));
}

#[test]
fn unproven_choices_keep_their_spelling() {
    for (written, read) in [
        ("if x > y { x } else { x }", "x.max(y)"),
        ("if x > g() { x } else { g() }", "x.max(g())"),
        ("if p { 2 } else { 0 }", "p as u32"),
        ("x.max(y)", "x.min(y)"),
        ("std::cmp::max(x, g())", "x.max(g())"),
        ("std::cmp::max(5, x)", "x.max(5)"),
        ("std::cmp::other(x, y)", "x.max(y)"),
        ("if p { x } else { y }", "x.max(y)"),
    ] {
        separate(&pair(written), &pair(read));
    }
    for (written, read) in [
        (
            "fn f(x: f64, y: f64) -> f64 { if x > y { x } else { y } }",
            "fn f(x: f64, y: f64) -> f64 { x.max(y) }",
        ),
        (
            "fn f(x: T, y: T) -> T { if x > y { x } else { y } }",
            "fn f(x: T, y: T) -> T { x.max(y) }",
        ),
        (
            "#![no_implicit_prelude] fn f(x: u32, y: u32) -> u32 { if x > y { x } else { y } }",
            "#![no_implicit_prelude] fn f(x: u32, y: u32) -> u32 { x.max(y) }",
        ),
        (
            "#![no_implicit_prelude] fn f(x: u32, y: u32) -> u32 { x.max(y) }",
            "#![no_implicit_prelude] fn f(x: u32, y: u32) -> u32 { y.max(x) }",
        ),
        (
            "struct Ord; fn f(x: u32, y: u32) -> u32 { x.max(y) }",
            "struct Ord; fn f(x: u32, y: u32) -> u32 { y.max(x) }",
        ),
        (
            "mod std { pub mod cmp { pub fn max(a: u32, _b: u32) -> u32 { a } } } fn f(x: u32, y: u32) -> u32 { std::cmp::max(x, y) }",
            "mod std { pub mod cmp { pub fn max(a: u32, _b: u32) -> u32 { a } } } fn f(x: u32, y: u32) -> u32 { x.max(y) }",
        ),
        (
            "use m::*; fn f(x: u32, y: u32) -> u32 { std::cmp::max(x, y) }",
            "use m::*; fn f(x: u32, y: u32) -> u32 { x.max(y) }",
        ),
    ] {
        separate(written, read);
    }
}
