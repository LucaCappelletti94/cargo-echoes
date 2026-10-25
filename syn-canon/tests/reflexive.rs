//! Reflexive equality under resolved total scalar proofs.

#[path = "support/view.rs"]
mod view;

use syn_canon::ContextKind;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, folded: &str) {
    assert_eq!(
        syn_canon::canonicalize(parse(written)),
        syn_canon::canonicalize(parse(folded)),
        "{written}\n{folded}"
    );
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

#[test]
fn reflexive_equality_folds_existing_primitive_domains() {
    for ty in [
        "bool", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
    ] {
        equivalent(
            &format!("fn f(x: {ty}) -> bool {{ x == x }}"),
            &format!("fn f(x: {ty}) -> bool {{ true }}"),
        );
    }
}

#[test]
fn reflexive_total_expressions_use_resolved_scalar_identity() {
    for expression in [
        "x & 7u32",
        "x < 7u32",
        "x << 1u32",
        "if x < 7u32 { 1u32 } else { 2u32 }",
    ] {
        equivalent(
            &format!("fn f(x: u32) -> bool {{ ({expression}) == ({expression}) }}"),
            "fn f(x: u32) -> bool { true }",
        );
    }
}

#[test]
fn equal_total_labels_can_have_different_operand_spelling() {
    equivalent(
        "fn f(x: u32, y: u32) -> bool { (x & y) == (y & x) }",
        "fn f(x: u32, y: u32) -> bool { true }",
    );
}

#[test]
fn reflexive_total_helpers_preserve_failing_source() {
    struct ConditionGroups;
    impl syn::visit_mut::VisitMut for ConditionGroups {
        fn visit_expr_if_mut(&mut self, branch: &mut syn::ExprIf) {
            syn::visit_mut::visit_expr_if_mut(self, branch);
            let condition = core::mem::replace(
                &mut *branch.cond,
                syn::Expr::Verbatim(proc_macro2::TokenStream::new()),
            );
            *branch.cond = syn::Expr::Group(syn::ExprGroup {
                attrs: Vec::new(),
                group_token: syn::token::Group::default(),
                expr: Box::new(condition),
            });
        }
    }
    equivalent(
        "fn helper(x: u32) -> u32 { x & 7u32 } fn f(x: u32) -> bool { helper(x) == helper(x) }",
        "fn helper(x: u32) -> u32 { x & 7u32 } fn f(x: u32) -> bool { true }",
    );
    equivalent(
        "fn helper(x: u32) -> bool { x == x } fn f(x: u32) -> bool { helper(x) == helper(x) }",
        "fn helper(x: u32) -> bool { true } fn f(x: u32) -> bool { true }",
    );
    let helpers = "fn helper(x: u32) -> u32 { x & 7u32 } fn flag(x: u32) -> bool { x < 7u32 }";
    for expression in [
        "helper(x) & 3u32",
        "!flag(x)",
        "if flag(x) { helper(x) } else { 1u32 }",
    ] {
        equivalent(
            &format!("{helpers} fn f(x: u32) -> bool {{ ({expression}) == ({expression}) }}"),
            &format!("{helpers} fn f(x: u32) -> bool {{ true }}"),
        );
    }
    let expression = "if !flag(x) { 1u32 } else { 2u32 }";
    let mut grouped = parse(&format!(
        "{helpers} fn f(x: u32) -> bool {{ ({expression}) == ({expression}) }}"
    ));
    syn::visit_mut::VisitMut::visit_file_mut(&mut ConditionGroups, &mut grouped);
    assert_eq!(
        syn_canon::canonicalize(grouped),
        syn_canon::canonicalize(parse(&format!("{helpers} fn f(x: u32) -> bool {{ true }}"))),
    );
}

#[test]
fn reflexive_producer_reads_preserve_producer_effects() {
    equivalent(
        "fn f(x: u32) -> bool { read(); x == x }",
        "fn f(x: u32) -> bool { read(); true }",
    );
    separate(
        "fn f(x: u32) -> bool { read(); x == x }",
        "fn f(x: u32) -> bool { true }",
    );
}

#[test]
fn distinct_origins_and_shadowed_values_keep_equality() {
    for source in [
        "fn f(x: u32, y: u32) -> bool { x == y }",
        "fn f(x: u32) -> bool { let y = x; let x: u32 = read(); x == y }",
    ] {
        separate(source, &source.replace("x == y", "true"));
    }
}

#[test]
fn unproven_domains_and_type_constraints_keep_equality() {
    for ty in ["f32", "f64", "usize", "isize", "char", "String", "Unknown"] {
        separate(
            &format!("fn f(x: {ty}) -> bool {{ x == x }}"),
            &format!("fn f(x: {ty}) -> bool {{ true }}"),
        );
    }
    for source in [
        "fn f() -> bool { let x = read(); x == x }",
        "fn f() -> bool { let x: u32 = read(); x == x }",
        "type u32 = Unknown; fn f(x: u32) -> bool { x == x }",
        "fn f() -> bool { let x = 1; x == x }",
        "fn f<T: PartialEq>(x: T) -> bool { x == x }",
    ] {
        separate(source, &source.replace("x == x", "true"));
    }
}

#[test]
fn repeated_effectful_or_partial_expressions_keep_equality() {
    for expression in ["read()", "x + 1u32", "x / 0u32", "x << 32u32", "*read()"] {
        separate(
            &format!("fn f(x: u32) -> bool {{ ({expression}) == ({expression}) }}"),
            "fn f(x: u32) -> bool { true }",
        );
    }
}

#[test]
fn borrowed_comparison_storage_keeps_its_expression() {
    separate(
        "fn f(x: u32) -> &'static bool { &(x == x) }",
        "fn f(x: u32) -> &'static bool { &true }",
    );
}

#[test]
fn observed_comparisons_keep_their_source() {
    for source in [
        "#[observe] fn f(x: u32) -> bool { x == x }",
        "fn f(x: u32) -> bool { #[observe] (x == x) }",
        "fn f(x: u32) { unknown!(x == x); }",
        "fn f(x: u32) { stringify!(x == x); }",
    ] {
        separate(source, &source.replace("x == x", "true"));
    }
    separate(
        "fn helper(x: u32) -> u32 { x & 7u32 } fn f(x: u32) -> bool { helper(#[observe] x) == helper(#[observe] x) }",
        "fn helper(x: u32) -> u32 { x & 7u32 } fn f(x: u32) -> bool { true }",
    );
}

#[test]
fn function_views_fold_proven_values_and_lexical_bodies_keep_unknown_types() {
    let file = parse("fn first(x: u32) -> bool { x == x } fn second(y: u32) -> bool { true }");
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file =
        parse("fn first(x: Unknown) -> bool { x == x } fn second(y: Unknown) -> bool { true }");
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
