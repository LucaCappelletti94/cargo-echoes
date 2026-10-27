//! Nested conditionals over proven conditions and boolean matches with a wildcard arm.

#[path = "support/view.rs"]
mod view;

use syn_canon::ContextKind;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, merged: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(merged));
    assert_eq!(left, right, "{written}\n{merged}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(merged)),
        "{written}\n{merged}"
    );
}

fn separate(written: &str, merged: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(merged)),
            "{written}\n{merged}"
        );
    }
}

fn function(body: &str) -> String {
    format!("fn f(p: bool, q: bool, r: bool, x: u32, y: u32) -> u32 {{ {body} }}")
}

#[test]
fn nested_conditionals_with_equal_else_branches_merge() {
    for (written, merged) in [
        (
            "if p { if q { x } else { y } } else { y }",
            "if p && q { x } else { y }",
        ),
        (
            "if p { if q { x } else { read() } } else { read() }",
            "if p && q { x } else { read() }",
        ),
        (
            "if p { if q { read() } else { y } } else { y }",
            "if q && p { read() } else { y }",
        ),
        (
            "if x < y { if p { x } else { y } } else { y }",
            "if p && x < y { x } else { y }",
        ),
        (
            "if p { if q { if r { x } else { y } } else { y } } else { y }",
            "if p && q && r { x } else { y }",
        ),
    ] {
        equivalent(&function(written), &function(merged));
    }
}

#[test]
fn nested_conditionals_without_else_merge() {
    for (written, merged) in [
        ("if p { if q { mark(); } } x", "if p && q { mark(); } x"),
        (
            "if p { if q { if r { mark(); } } } x",
            "if r && (q && p) { mark(); } x",
        ),
        (
            "let mut v = x; if p { if q { v = y; } } v",
            "let mut v = x; if p && q { v = y; } v",
        ),
    ] {
        equivalent(&function(written), &function(merged));
    }
}

#[test]
fn other_shapes_keep_the_nesting() {
    for (written, merged) in [
        (
            "if p { if q { x } else { y } } else { x }",
            "if p && q { x } else { y }",
        ),
        (
            "if p { if q { x } else { y } } x",
            "if p && q { x } else { y }",
        ),
        (
            "if p { if q { mark(); } } else { other(); } x",
            "if p && q { mark(); } else { other(); } x",
        ),
        (
            "if p { mark(); if q { other(); } } x",
            "if p && q { mark(); other(); } x",
        ),
        ("if p { if q { mark(); }; } x", "if p && q { mark(); } x"),
        (
            "if p { if let Some(v) = Some(x) { mark(); } } x",
            "if p && let Some(v) = Some(x) { mark(); } x",
        ),
        (
            "if read() { if q { mark(); } } x",
            "if read() && q { mark(); } x",
        ),
        (
            "if p { if read() { mark(); } } x",
            "if p && read() { mark(); } x",
        ),
    ] {
        separate(&function(written), &function(merged));
    }
    separate(
        "fn f(p: Unknown, q: bool) { if p { if q { mark(); } } }",
        "fn f(p: Unknown, q: bool) { if p && q { mark(); } }",
    );
}

#[test]
fn wildcard_arms_stand_for_the_other_boolean() {
    for (written, conditional) in [
        ("match p { true => x, _ => y }", "if p { x } else { y }"),
        ("match p { false => y, _ => x }", "if p { x } else { y }"),
        (
            "match q && p { true => x, _ => y }",
            "if p && q { x } else { y }",
        ),
    ] {
        equivalent(&function(written), &function(conditional));
    }
    for (written, conditional) in [
        ("match p { _ => x, true => y }", "if p { y } else { x }"),
        (
            "match p { true if q => x, _ => y }",
            "if p && q { x } else { y }",
        ),
        ("match p { true => x, v => y }", "if p { x } else { y }"),
    ] {
        separate(&function(written), &function(conditional));
    }
}

#[test]
fn observed_conditionals_keep_their_nesting() {
    for source in [
        "#[observe] fn f(p: bool, q: bool) { if p { if q { mark(); } } }",
        "fn f(p: bool, q: bool) { #[observe] if p { if q { mark(); } } }",
        "fn f(p: bool, q: bool) { if p { #[observe] if q { mark(); } } }",
        "fn f(p: bool, q: bool) { unknown!(if p { if q { mark(); } }); }",
    ] {
        let merged = source
            .replace("#[observe] if q { mark(); }", "#[observe] mark();")
            .replace("if p { if q { mark(); } }", "if p && q { mark(); }")
            .replace(
                "if p { #[observe] mark(); }",
                "if p && q { #[observe] mark(); }",
            );
        separate(source, &merged);
    }
}

#[test]
fn function_views_merge_proven_conditions_and_lexical_bodies_keep_unknown_types() {
    let file = parse(
        "fn first(p: bool, q: bool) { if p { if q { mark(); } } } fn second(a: bool, b: bool) { if a && b { mark(); } }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "fn first(p: Unknown, q: Unknown) { if p { if q { mark(); } } } fn second(p: Unknown, q: Unknown) { if p && q { mark(); } }",
    );
    assert_ne!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let mut first = None;
    let mut second = None;
    syn_canon::contexts(&file, &mut |context, form| {
        if context.kind == ContextKind::FunctionBody {
            match context.item {
                "first" => first = Some(form),
                "second" => second = Some(form),
                _ => {}
            }
        }
    });
    assert_ne!(
        first.expect("first function body"),
        second.expect("second function body")
    );
}
