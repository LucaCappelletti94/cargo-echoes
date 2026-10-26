//! Operand order of commutative operators and inherent methods over proven fixed-width integers.

#[path = "support/view.rs"]
mod view;

use syn_canon::ContextKind;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, reordered: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(reordered));
    assert_eq!(left, right, "{written}\n{reordered}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(reordered)),
        "{written}\n{reordered}"
    );
}

fn separate(written: &str, reordered: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(reordered)),
            "{written}\n{reordered}"
        );
    }
}

const INTEGERS: [&str; 10] = [
    "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
];

fn function(ty: &str, body: &str) -> String {
    format!("fn f(x: {ty}, y: {ty}, z: {ty}) -> {ty} {{ {body} }}")
}

#[test]
fn addition_and_multiplication_take_one_operand_order() {
    for ty in INTEGERS {
        for (written, reordered) in [("x + y", "y + x"), ("x * y", "y * x")] {
            equivalent(&function(ty, written), &function(ty, reordered));
        }
    }
}

#[test]
fn literals_nested_values_and_helpers_take_one_operand_order() {
    for (written, reordered) in [
        ("x + 1", "1 + x"),
        ("3u32 * x", "x * 3u32"),
        ("(x ^ y) + z", "z + (y ^ x)"),
        ("(x ^ y) * z", "z * (y ^ x)"),
        ("x.wrapping_mul(y) + z", "z + y.wrapping_mul(x)"),
    ] {
        equivalent(&function("u32", written), &function("u32", reordered));
    }
    let helper = "fn h(x: u32) -> u32 { x & 7u32 }";
    equivalent(
        &format!("{helper} fn f(x: u32, y: u32) -> u32 {{ h(x) + y }}"),
        &format!("{helper} fn f(x: u32, y: u32) -> u32 {{ y + h(x) }}"),
    );
}

#[test]
fn commutative_inherent_methods_take_one_operand_order() {
    for ty in INTEGERS {
        for method in [
            "wrapping_add",
            "wrapping_mul",
            "saturating_add",
            "saturating_mul",
        ] {
            equivalent(
                &function(ty, &format!("x.{method}(y)")),
                &function(ty, &format!("y.{method}(x)")),
            );
        }
    }
    for (signed, unsigned) in [("i8", "u8"), ("i32", "u32"), ("u64", "u64")] {
        equivalent(
            &format!("fn f(x: {signed}, y: {signed}) -> {unsigned} {{ x.abs_diff(y) }}"),
            &format!("fn f(x: {signed}, y: {signed}) -> {unsigned} {{ y.abs_diff(x) }}"),
        );
    }
}

#[test]
fn different_methods_and_operators_stay_apart() {
    for (written, reordered) in [
        ("x.wrapping_add(y)", "x.wrapping_mul(y)"),
        ("x.saturating_add(y)", "x.wrapping_add(y)"),
        ("x + y", "x * y"),
    ] {
        separate(&function("u32", written), &function("u32", reordered));
    }
}

#[test]
fn ordered_operators_regrouping_and_partial_values_keep_their_source() {
    for (written, reordered) in [
        ("x - y", "y - x"),
        ("x / y", "y / x"),
        ("x % y", "y % x"),
        ("(x + y) + z", "x + (y + z)"),
        ("(x * y) * z", "x * (y * z)"),
        ("x.wrapping_sub(y)", "y.wrapping_sub(x)"),
    ] {
        separate(&function("i32", written), &function("i32", reordered));
    }
    separate(
        "fn f(x: u32, y: u32) -> bool { (x + y) == (y + x) }",
        "fn f(x: u32, y: u32) -> bool { true }",
    );
}

#[test]
fn unproven_operands_and_receivers_keep_their_order() {
    for ty in ["f32", "usize", "isize", "Unknown"] {
        separate(&function(ty, "x + y"), &function(ty, "y + x"));
        separate(
            &function(ty, "x.wrapping_mul(y)"),
            &function(ty, "y.wrapping_mul(x)"),
        );
    }
    for (written, reordered) in [
        ("read() + x", "x + read()"),
        ("x + *r", "*r + x"),
        ("u32::wrapping_mul(x, y)", "u32::wrapping_mul(y, x)"),
    ] {
        separate(
            &format!("fn f(x: u32, y: u32, r: &u32) -> u32 {{ {written} }}"),
            &format!("fn f(x: u32, y: u32, r: &u32) -> u32 {{ {reordered} }}"),
        );
    }
    separate(
        "struct S; impl S { fn wrapping_mul(&self, other: &S) -> u32 { 0 } } fn f(a: S, b: S) -> u32 { a.wrapping_mul(&b) }",
        "struct S; impl S { fn wrapping_mul(&self, other: &S) -> u32 { 0 } } fn f(a: S, b: S) -> u32 { b.wrapping_mul(&a) }",
    );
    for (written, reordered) in [
        (
            "fn f(x: u32, y: u64) -> u32 { x.wrapping_mul(y as u32) ^ x.wrapping_mul(1) }",
            "fn f(x: u32, y: u64) -> u32 { x.wrapping_mul(1) ^ x.wrapping_mul(y as u32) }",
        ),
        (
            "fn f(x: u32, y: u64) -> u64 { y.saturating_add(x) }",
            "fn f(x: u32, y: u64) -> u64 { x.saturating_add(y) }",
        ),
        (
            "fn f(p: bool, q: bool) -> bool { p.wrapping_add(q) }",
            "fn f(p: bool, q: bool) -> bool { q.wrapping_add(p) }",
        ),
    ] {
        separate(written, reordered);
    }
}

#[test]
fn observed_operands_keep_their_order() {
    for source in [
        "#[observe] fn f(x: u32, y: u32) -> u32 { y + x }",
        "fn f(x: u32, y: u32) -> u32 { #[observe] (y + x) }",
        "fn f(x: u32, y: u32) { unknown!(y + x); }",
    ] {
        separate(source, &source.replace("y + x", "x + y"));
    }
}

#[test]
fn function_views_order_proven_operands_and_lexical_bodies_keep_unknown_types() {
    let file = parse(
        "fn first(x: u32, y: u32) -> u32 { y * x } fn second(a: u32, b: u32) -> u32 { a * b }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "fn first(x: u32, y: Unknown) -> u32 { (y + x) ^ y } fn second(x: u32, y: Unknown) -> u32 { (x + y) ^ y }",
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
