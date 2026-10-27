//! Early returns before a tail expression read as `else` branches.

#[path = "support/view.rs"]
mod view;

use syn_canon::ContextKind;

const DROP_GUARD: &str = r#"
use std::io::Write as _;
struct D(&'static str);
impl D { fn value(&self) -> u32 { 0 } }
impl Drop for D {
    fn drop(&mut self) { let _ = writeln!(std::io::stdout(), "drop {}", self.0); }
}
"#;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, branched: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(branched));
    assert_eq!(left, right, "{written}\n{branched}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(branched)),
        "{written}\n{branched}"
    );
}

fn separate(written: &str, branched: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(branched)),
            "{written}\n{branched}"
        );
    }
}

fn function(body: &str) -> String {
    format!("fn f(p: bool, q: bool, x: u32, y: u32) -> u32 {{ {body} }}")
}

#[test]
fn early_returns_before_a_tail_match_branches() {
    for (written, branched) in [
        ("if p { return x; } y", "if p { x } else { y }"),
        ("if p { return x } y", "if p { x } else { y }"),
        ("if p { return x; }; y", "if p { x } else { y }"),
        ("if p { return x; } return y;", "if p { x } else { y }"),
        (
            "if p { mark(); return x; } y",
            "if p { mark(); x } else { y }",
        ),
        (
            "if p { let z: u32 = make(); return z; } y",
            "if p { let z: u32 = make(); z } else { y }",
        ),
        (
            "let z: u32 = make(); if p { return z; } y",
            "let z: u32 = make(); if p { z } else { y }",
        ),
        (
            "if p { return x; } if q { return y; } x + y",
            "if p { x } else if q { y } else { x + y }",
        ),
        (
            "if p { return x; } if q { y } else { x }",
            "if p { x } else if q { y } else { x }",
        ),
        (
            "if p { return x; } match y { 0 => x, _ => y }",
            "if p { x } else { match y { 0 => x, _ => y } }",
        ),
        ("if p { return 1; } 2", "if p { 1 } else { 2 }"),
    ] {
        equivalent(&function(written), &function(branched));
    }
}

#[test]
fn nested_tail_blocks_and_closures_match_branches() {
    equivalent(
        &function("match y { 0 => { if p { return x; } y } _ => x }"),
        &function("match y { 0 => { if p { x } else { y } } _ => x }"),
    );
    equivalent(
        "fn f() { let _ = |p: bool, x: u32, y: u32| -> u32 { if p { return x; } y }; }",
        "fn f() { let _ = |p: bool, x: u32, y: u32| -> u32 { if p { x } else { y } }; }",
    );
}

#[test]
fn other_shapes_keep_the_early_return() {
    for (written, branched) in [
        ("if p { return x; } make()", "if p { x } else { make() }"),
        ("if test() { return x; } y", "if test() { x } else { y }"),
        (
            "if p { return x; } mark(); y",
            "if p { x } else { mark(); y }",
        ),
        ("if p { mark(); } y", "if p { mark(); y } else { y }"),
        ("if p { return make(); } y", "if p { make() } else { y }"),
        (
            "'a: { if p { return x; } y }",
            "'a: { if p { x } else { y } }",
        ),
        (
            "loop { if p { return x; } break y; }",
            "loop { if p { break x; } else { break y; } }",
        ),
    ] {
        separate(&function(written), &function(branched));
    }
    separate(
        "fn f(o: Option<u32>, y: u32) -> u32 { if let Some(z) = o { return z; } y }",
        "fn f(o: Option<u32>, y: u32) -> u32 { if let Some(z) = o { z } else { y } }",
    );
}

#[test]
fn async_bodies_keep_the_early_return() {
    for template in [
        "async fn f(p: bool, x: u32, y: u32) -> u32 { BODY }",
        "fn f(p: bool, x: u32, y: u32) -> impl Future<Output = u32> { async move { BODY } }",
        "fn f() { let _ = async |p: bool, x: u32, y: u32| -> u32 { BODY }; }",
    ] {
        separate(
            &template.replace("BODY", "if p { return x; } y"),
            &template.replace("BODY", "if p { x } else { y }"),
        );
    }
}

#[test]
fn destructor_temporaries_in_the_tail_keep_the_early_return() {
    separate(
        &format!(
            "{DROP_GUARD} fn f(p: bool) -> u32 {{ let local = D(\"local\"); if p {{ return 1; }} D(\"temp\").value() }}"
        ),
        &format!(
            "{DROP_GUARD} fn f(p: bool) -> u32 {{ let local = D(\"local\"); if p {{ 1 }} else {{ D(\"temp\").value() }} }}"
        ),
    );
}

#[test]
fn observed_bodies_keep_the_early_return() {
    for template in [
        "#[observe] fn f(p: bool, x: u32, y: u32) -> u32 { BODY }",
        "fn f(p: bool, x: u32, y: u32) -> u32 { observe!({ BODY }) }",
    ] {
        separate(
            &template.replace("BODY", "if p { return x; } y"),
            &template.replace("BODY", "if p { x } else { y }"),
        );
    }
}

#[test]
fn function_views_match_proven_branches_and_lexical_bodies_keep_unknown_types() {
    let file = parse(
        "fn first(p: bool, x: u32, y: u32) -> u32 { if p { return x; } y } fn second(a: bool, b: u32, c: u32) -> u32 { if a { b } else { c } }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "fn first(p: Unknown, x: Unknown, y: Unknown) -> Unknown { if p { return x; } y } fn second(p: Unknown, x: Unknown, y: Unknown) -> Unknown { if p { x } else { y } }",
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
