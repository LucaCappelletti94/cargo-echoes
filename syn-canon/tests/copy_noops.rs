//! Copies of proven primitives that dereference a borrow, compare borrows or clone.

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

fn function(body: &str) -> String {
    format!("fn f(p: bool, x: u32, y: u32) -> u32 {{ {body} }}")
}

#[test]
fn copies_of_proven_primitives_reduce_to_the_value() {
    for (written, canonical) in [
        ("*&x", "x"),
        ("x.clone()", "x"),
        ("x.clone() + y", "x + y"),
        ("*&x * y.clone()", "x * y"),
        ("if p.clone() { x } else { y }", "if p { x } else { y }"),
        ("if &x == &y { x } else { y }", "if x == y { x } else { y }"),
        ("if &x != &y { x } else { y }", "if x != y { x } else { y }"),
        ("let z = x ^ y; z.clone() + z", "let z = x ^ y; z + z"),
    ] {
        equivalent(&function(written), &function(canonical));
    }
    equivalent(
        "use std::collections::HashMap; fn f(x: u32) -> u32 { x.clone() }",
        "use std::collections::HashMap; fn f(x: u32) -> u32 { x }",
    );
}

#[test]
fn other_copies_keep_their_spelling() {
    for (written, canonical) in [
        ("*&mut x", "x"),
        ("if &x < &y { x } else { y }", "if x < y { x } else { y }"),
        (
            "if &mut x == &mut 1u32 { x } else { y }",
            "if x == 1u32 { x } else { y }",
        ),
        (
            "if &x == &make() { x } else { y }",
            "if x == make() { x } else { y }",
        ),
    ] {
        separate(
            &format!("fn f(mut x: u32, y: u32) -> u32 {{ {written} }}"),
            &format!("fn f(mut x: u32, y: u32) -> u32 {{ {canonical} }}"),
        );
    }
    for (written, canonical) in [
        (
            "fn f(s: String) -> String { s.clone() }",
            "fn f(s: String) -> String { s }",
        ),
        (
            "fn f(x: Unknown) -> Unknown { x.clone() }",
            "fn f(x: Unknown) -> Unknown { x }",
        ),
        (
            "fn f(x: &u32) -> u32 { *&*x }",
            "fn f(x: &u32) -> u32 { *x }",
        ),
        (
            "fn f(a: String, b: String) -> bool { &a == &b }",
            "fn f(a: String, b: String) -> bool { a == b }",
        ),
    ] {
        separate(written, canonical);
    }
}

#[test]
fn clone_needs_standard_imports_and_no_user_clone() {
    for prefix in [
        "trait Dup { fn clone(self) -> Self; } impl Dup for u32 { fn clone(self) -> Self { self + 1 } }",
        "use other::Dup;",
        "use other::*;",
        "#![no_implicit_prelude]",
    ] {
        separate(
            &format!("{prefix} fn f(x: u32) -> u32 {{ x.clone() }}"),
            &format!("{prefix} fn f(x: u32) -> u32 {{ x }}"),
        );
    }
    separate(
        "fn f(x: u32) -> u32 { use other::Dup; x.clone() }",
        "fn f(x: u32) -> u32 { use other::Dup; x }",
    );
    equivalent(
        "use other::Dup; fn f(x: u32) -> u32 { *&x }",
        "use other::Dup; fn f(x: u32) -> u32 { x }",
    );
    separate(
        "#[observe] fn f(x: u32) -> u32 { x.clone() }",
        "#[observe] fn f(x: u32) -> u32 { x }",
    );
}

#[test]
fn function_views_follow_the_module_imports() {
    let file = parse("fn first(x: u32) -> u32 { x.clone() } fn second(y: u32) -> u32 { *&y }");
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "use other::Dup; fn first(x: u32) -> u32 { x.clone() } fn second(y: u32) -> u32 { y }",
    );
    assert_ne!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
}
