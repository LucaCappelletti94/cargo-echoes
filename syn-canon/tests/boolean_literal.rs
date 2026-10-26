//! Boolean comparisons with a literal over proven boolean values.

#[path = "support/view.rs"]
mod view;

use syn_canon::ContextKind;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, folded: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(folded));
    assert_eq!(left, right, "{written}\n{folded}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(folded)),
        "{written}\n{folded}"
    );
}

fn separate(written: &str, folded: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(folded)),
            "{written}\n{folded}"
        );
    }
}

fn function(body: &str) -> String {
    format!("fn f(p: bool, q: bool, x: u32, y: u32) -> bool {{ {body} }}")
}

#[test]
fn literal_comparisons_fold_in_both_operand_orders() {
    for (written, folded) in [
        ("p == false", "!p"),
        ("false == p", "!p"),
        ("p != true", "!p"),
        ("true != p", "!p"),
        ("p != false", "p"),
        ("false != p", "p"),
        ("p == true", "p"),
        ("true == p", "p"),
    ] {
        equivalent(&function(written), &function(folded));
    }
}

#[test]
fn literal_operands_fold_to_the_comparison_value() {
    for (written, folded) in [
        ("true == false", "false"),
        ("false == false", "true"),
        ("true != true", "false"),
        ("false != true", "true"),
    ] {
        equivalent(&function(written), &function(folded));
    }
}

#[test]
fn structured_operands_take_their_complement() {
    for (written, folded) in [
        ("(x < y) == false", "x >= y"),
        ("(x == y) != true", "x != y"),
        ("(x != y) == false", "x == y"),
        ("(p && q) == false", "!p || !q"),
        ("(p || q) != true", "!p && !q"),
        ("!p == false", "p"),
        ("!p != false", "!p"),
        ("(p & q) == false", "!(p & q)"),
        ("(p ^ q) != false", "p ^ q"),
        ("(x & 7u32 == y) != true", "x & 7u32 != y"),
    ] {
        equivalent(&function(written), &function(folded));
    }
}

#[test]
fn negated_operands_keep_their_grouping() {
    separate(&function("(p & q) == false"), &function("!p & q"));
    separate(&function("(p | q) != true"), &function("!p | q"));
}

#[test]
fn accepted_total_helpers_fold_like_values() {
    let helper = "fn h(x: u32) -> bool { x < 7u32 }";
    for (written, folded) in [
        ("h(x) == false", "!h(x)"),
        ("h(x) != true", "!h(x)"),
        ("h(x) != false", "h(x)"),
        ("h(x) != (x == 3u32)", "(x == 3u32) != h(x)"),
    ] {
        equivalent(
            &format!("{helper} fn f(x: u32) -> bool {{ {written} }}"),
            &format!("{helper} fn f(x: u32) -> bool {{ {folded} }}"),
        );
    }
}

#[test]
fn surrounding_effects_stay_in_place() {
    equivalent(&function("read(); p == false"), &function("read(); !p"));
    equivalent(
        "fn f(p: bool) -> u32 { if p != true { read() } else { 2u32 } }",
        "fn f(p: bool) -> u32 { if !p { read() } else { 2u32 } }",
    );
}

#[test]
fn conditions_read_the_reduced_operand() {
    let typed = |body: &str| format!("fn f(p: bool, x: u32) -> u32 {{ {body} }}");
    for (written, folded) in [
        (
            "if p == false { x } else { 2u32 }",
            "if p { 2u32 } else { x }",
        ),
        (
            "if false != p { x } else { 2u32 }",
            "if p { x } else { 2u32 }",
        ),
        (
            "if p != true { read() } else { x }",
            "if !p { read() } else { x }",
        ),
        ("if p == false { read(); } x", "if !p { read(); } x"),
        (
            "if x == 1u32 { 1u32 } else if p == false { read() } else { x }",
            "if x == 1u32 { 1u32 } else if p { x } else { read() }",
        ),
    ] {
        equivalent(&typed(written), &typed(folded));
    }
}

#[test]
fn unproven_operands_keep_the_comparison() {
    for (written, folded) in [
        ("z == false", "!z"),
        ("z != true", "!z"),
        ("z != false", "z"),
        ("read() == false", "!read()"),
        ("read() != false", "read()"),
    ] {
        separate(
            &format!("fn f(z: Unknown) -> bool {{ {written} }}"),
            &format!("fn f(z: Unknown) -> bool {{ {folded} }}"),
        );
    }
    separate(
        "struct S; impl PartialEq<bool> for S { fn eq(&self, _: &bool) -> bool { true } } fn f(s: S) -> bool { s == false }",
        "struct S; impl PartialEq<bool> for S { fn eq(&self, _: &bool) -> bool { true } } fn f(s: S) -> bool { !s }",
    );
    for ty in ["u32", "i8", "usize", "char", "f32"] {
        separate(
            &format!("fn f(x: {ty}) -> bool {{ x != false }}"),
            &format!("fn f(x: {ty}) -> bool {{ x }}"),
        );
    }
}

#[test]
fn observed_comparisons_keep_their_source() {
    for source in [
        "#[observe] fn f(p: bool) -> bool { p == false }",
        "fn f(p: bool) -> bool { #[observe] (p == false) }",
        "fn f(p: bool) { unknown!(p == false); }",
        "fn f(p: bool) { stringify!(p == false); }",
    ] {
        separate(source, &source.replace("p == false", "!p"));
    }
    separate(
        "fn f(p: bool) -> bool { (#[observe] p) == false }",
        "fn f(p: bool) -> bool { !(#[observe] p) }",
    );
}

#[test]
fn function_views_fold_proven_values_and_lexical_bodies_keep_unknown_types() {
    let file = parse(
        "fn first(p: bool, q: bool) -> bool { p == false && q != true } fn second(a: bool, b: bool) -> bool { !a && !b }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file =
        parse("fn first(p: Unknown) -> bool { p == false } fn second(p: Unknown) -> bool { !p }");
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
