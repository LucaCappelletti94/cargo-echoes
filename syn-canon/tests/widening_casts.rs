//! Lossless widening casts and standard `From` conversions between proven primitives.

#[path = "support/view.rs"]
mod view;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, canonical: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(canonical));
    assert_eq!(left, right, "{written}\n{canonical}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(canonical)),
        "{written}\n{canonical}"
    );
}

fn separate(written: &str, canonical: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(canonical)),
            "{written}\n{canonical}"
        );
    }
}

fn convert(source: &str, target: &str, body: &str) -> String {
    format!("fn f(x: {source}) -> {target} {{ {body} }}")
}

#[test]
fn lossless_casts_match_from_conversions() {
    for (source, target) in [
        ("u8", "u16"),
        ("u8", "i16"),
        ("u16", "u32"),
        ("u32", "u64"),
        ("u32", "i64"),
        ("u64", "u128"),
        ("i8", "i16"),
        ("i32", "i64"),
        ("i64", "i128"),
        ("bool", "u8"),
        ("bool", "i32"),
    ] {
        equivalent(
            &convert(source, target, &format!("x as {target}")),
            &convert(source, target, &format!("{target}::from(x)")),
        );
    }
}

#[test]
fn identity_casts_and_conversions_reduce_to_the_operand() {
    for body in ["x as u32", "u32::from(x)"] {
        equivalent(&convert("u32", "u32", body), &convert("u32", "u32", "x"));
    }
}

#[test]
fn conversions_inside_expressions_and_of_locals_match() {
    equivalent(
        "fn f(x: u32, y: u64) -> u64 { y + x as u64 }",
        "fn f(x: u32, y: u64) -> u64 { y + u64::from(x) }",
    );
    equivalent(
        "fn f(x: u16) -> u64 { let z = x; u64::from(z) }",
        "fn f(x: u16) -> u64 { let z = x; z as u64 }",
    );
}

#[test]
fn lossy_and_unproven_conversions_stay_apart() {
    for (source, target) in [
        ("u32", "u16"),
        ("u32", "i32"),
        ("i32", "u64"),
        ("i8", "u16"),
        ("u32", "usize"),
        ("u8", "usize"),
        ("u8", "bool"),
        ("Unknown", "u64"),
    ] {
        separate(
            &convert(source, target, &format!("x as {target}")),
            &convert(source, target, &format!("{target}::from(x)")),
        );
    }
    for prefix in [
        "#![no_std]",
        "#![no_implicit_prelude]",
        "use custom::From;",
        "use custom::u64;",
        "use other::*;",
    ] {
        separate(
            &format!("{prefix} fn f(x: u32) -> u64 {{ x as u64 }}"),
            &format!("{prefix} fn f(x: u32) -> u64 {{ u64::from(x) }}"),
        );
    }
    separate(
        "#[observe] fn f(x: u32) -> u64 { x as u64 }",
        "#[observe] fn f(x: u32) -> u64 { u64::from(x) }",
    );
    separate(
        "fn f(x: u32) -> u64 { <u64>::from(x) }",
        "fn f(x: u32) -> u64 { x as u64 }",
    );
    separate(
        "fn f(x: u32) -> u64 { u64::try_from(x).unwrap() }",
        "fn f(x: u32) -> u64 { x as u64 }",
    );
}

#[test]
fn function_views_match_proven_conversions() {
    let file =
        parse("fn first(x: u32) -> u64 { x as u64 } fn second(y: u32) -> u64 { u64::from(y) }");
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "#![no_std] fn first(x: u32) -> u64 { x as u64 } fn second(y: u32) -> u64 { u64::from(y) }",
    );
    assert_ne!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
}
