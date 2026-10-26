//! Literals, repetition, operand order and conditionals in `&&` and `||` over proven booleans.

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
    format!("fn f(p: bool, q: bool, r: bool, x: u32, y: u32) -> bool {{ {body} }}")
}

fn pairs(cases: &[(&str, &str)]) {
    for (written, folded) in cases {
        equivalent(&function(written), &function(folded));
    }
}

#[test]
fn identity_literals_leave_the_operand() {
    pairs(&[
        ("p && true", "p"),
        ("true && p", "p"),
        ("p || false", "p"),
        ("false || p", "p"),
        ("true && p && true", "p"),
        ("(x < y) && true", "x < y"),
        ("true && true", "true"),
        ("false || false", "false"),
    ]);
}

#[test]
fn absorbing_literals_decide_the_chain() {
    pairs(&[
        ("false && p", "false"),
        ("true || p", "true"),
        ("p && false", "false"),
        ("p || true", "true"),
        ("p && q && false", "false"),
        ("(x < y) || true", "true"),
    ]);
}

#[test]
fn repeated_operands_collapse() {
    pairs(&[
        ("p && p", "p"),
        ("p || p", "p"),
        ("p && q && p", "p && q"),
        ("(x < y) || (y > x)", "x < y"),
    ]);
}

#[test]
fn operand_order_follows_labels_across_one_operator() {
    pairs(&[
        ("q && p", "p && q"),
        ("q || p", "p || q"),
        ("r && (q && p)", "(p && q) && r"),
        ("x < y && p", "p && x < y"),
        ("(q || p) && r", "r && (p || q)"),
    ]);
    separate(&function("p && (q || r)"), &function("(p && q) || r"));
    separate(&function("p && q"), &function("p || q"));
}

#[test]
fn negated_chains_keep_a_canonical_order() {
    pairs(&[
        ("!(q && p)", "!p || !q"),
        ("!(x < y && p)", "x >= y || !p"),
        ("!(q || p || r)", "!r && !p && !q"),
    ]);
}

#[test]
fn literal_conditionals_read_as_logical_operators() {
    pairs(&[
        ("if p { q } else { false }", "p && q"),
        ("if p { true } else { q }", "p || q"),
        ("if p { false } else { q }", "!p && q"),
        ("if p { q } else { true }", "!p || q"),
        ("if x < y { q } else { false }", "q && x < y"),
        ("if !p { q } else { false }", "q && !p"),
    ]);
}

#[test]
fn accepted_total_helpers_order_like_values() {
    let helper = "fn h(x: u32) -> bool { x < 7u32 }";
    for (written, folded) in [
        ("h(x) && p", "p && h(x)"),
        ("h(x) || h(x)", "h(x)"),
        ("h(x) && false", "false"),
    ] {
        equivalent(
            &format!("{helper} fn f(p: bool, x: u32) -> bool {{ {written} }}"),
            &format!("{helper} fn f(p: bool, x: u32) -> bool {{ {folded} }}"),
        );
    }
}

#[test]
fn helpers_with_logical_bodies_stay_total() {
    let helper = "fn h(p: bool, q: bool) -> bool { q && (p || false) }";
    equivalent(
        &format!("{helper} fn f(p: bool, q: bool) -> bool {{ h(p, q) == h(p, q) }}"),
        &format!("{helper} fn f(p: bool, q: bool) -> bool {{ true }}"),
    );
}

#[test]
fn local_item_order_observes_logical_initializers_like_values() {
    let first = "fn f() -> bool { fn one() -> u32 { 1 } fn two() -> u32 { 2 } let c = (two() < one()) || (one() < two()); println!(\"{:?}\", c); c }";
    let second = "fn f() -> bool { fn two() -> u32 { 2 } fn one() -> u32 { 1 } let c = (one() < two()) || (two() < one()); println!(\"{:?}\", c); c }";
    equivalent(first, second);
    assert_eq!(
        view::view_of(&parse(first), "f"),
        view::view_of(&parse(second), "f")
    );
}

#[test]
fn long_chains_of_distinct_operands_match_in_any_order() {
    fn tree(leaves: &[usize]) -> String {
        if let [leaf] = leaves {
            return format!("a{leaf}");
        }
        let (left, right) = leaves.split_at(leaves.len() / 2);
        format!("({} && {})", tree(left), tree(right))
    }
    let count = 4096;
    let params = (0..count)
        .map(|i| format!("a{i}: bool, "))
        .collect::<Vec<_>>()
        .concat();
    let forward = tree(&(0..count).collect::<Vec<_>>());
    let reversed = tree(&(0..count).rev().collect::<Vec<_>>());
    let source = |body: &str| format!("fn f({params}) -> bool {{ {body} }}");
    equivalent(&source(&forward), &source(&reversed));
    equivalent(
        &source(&format!("{forward} && true")),
        &source(&format!("{reversed} && a0")),
    );
}

#[test]
fn effectful_and_partial_operands_keep_their_order() {
    for (written, folded) in [
        ("read() && p", "p && read()"),
        ("read() && false", "false"),
        ("false && read()", "false"),
        ("read() || read()", "read()"),
        ("x / y > 0u32 && p", "p && x / y > 0u32"),
        ("if p { read() } else { false }", "p && read()"),
        ("if p { let v = q; v } else { false }", "p && q"),
    ] {
        separate(&function(written), &function(folded));
    }
}

#[test]
fn unproven_operands_keep_their_order() {
    for (written, folded) in [("z && p", "p && z"), ("z && true", "z"), ("z || z", "z")] {
        separate(
            &format!("fn f(p: bool, z: Unknown) -> bool {{ {written} }}"),
            &format!("fn f(p: bool, z: Unknown) -> bool {{ {folded} }}"),
        );
    }
}

#[test]
fn borrowed_storage_keeps_its_operand() {
    separate(&function("*&(p && true)"), &function("*&p"));
    separate(
        "fn f(p: bool) -> &'static bool { &(p || false) }",
        "fn f(p: bool) -> &'static bool { &p }",
    );
}

#[test]
fn observed_operators_keep_their_source() {
    for source in [
        "#[observe] fn f(p: bool, q: bool) -> bool { q && p }",
        "fn f(p: bool, q: bool) -> bool { #[observe] (q && p) }",
        "fn f(p: bool, q: bool) { unknown!(q && p); }",
        "fn f(p: bool, q: bool) { stringify!(q && p); }",
    ] {
        separate(source, &source.replace("q && p", "p && q"));
    }
    separate(
        "fn f(p: bool, q: bool) -> bool { (#[observe] q) && p }",
        "fn f(p: bool, q: bool) -> bool { p && (#[observe] q) }",
    );
}

#[test]
fn function_views_fold_proven_values_and_lexical_bodies_keep_unknown_types() {
    let file = parse(
        "fn first(p: bool, q: bool) -> bool { (if q { p } else { false }) || false } fn second(a: bool, b: bool) -> bool { a && b }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "fn first(p: Unknown, q: bool) -> bool { p && q && true } fn second(p: Unknown, q: bool) -> bool { p && q }",
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
