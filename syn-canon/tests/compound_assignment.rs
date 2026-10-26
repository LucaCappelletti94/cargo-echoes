//! Compound assignment to proven fixed-width integer and `bool` locals.

#[path = "support/view.rs"]
mod view;

use syn_canon::ContextKind;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, expanded: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(expanded));
    assert_eq!(left, right, "{written}\n{expanded}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(expanded)),
        "{written}\n{expanded}"
    );
}

fn separate(written: &str, expanded: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(expanded)),
            "{written}\n{expanded}"
        );
    }
}

fn function(ty: &str, statement: &str) -> String {
    format!("fn f(mut x: {ty}, y: {ty}) -> {ty} {{ {statement}; x }}")
}

const INTEGER_OPERATORS: [(&str, &str); 10] = [
    ("+=", "+"),
    ("-=", "-"),
    ("*=", "*"),
    ("/=", "/"),
    ("%=", "%"),
    ("&=", "&"),
    ("|=", "|"),
    ("^=", "^"),
    ("<<=", "<<"),
    (">>=", ">>"),
];

#[test]
fn every_integer_operator_matches_its_expanded_assignment() {
    for ty in [
        "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
    ] {
        for (compound, plain) in INTEGER_OPERATORS {
            equivalent(
                &function(ty, &format!("x {compound} y")),
                &function(ty, &format!("x = x {plain} y")),
            );
        }
    }
}

#[test]
fn bitwise_operators_match_on_booleans() {
    for (compound, plain) in [("&=", "&"), ("|=", "|"), ("^=", "^")] {
        equivalent(
            &function("bool", &format!("x {compound} y")),
            &function("bool", &format!("x = x {plain} y")),
        );
    }
    separate(&function("bool", "x += y"), &function("bool", "x = x + y"));
}

#[test]
fn literals_locals_and_shift_amounts_of_another_type_match() {
    for (written, expanded) in [
        (
            "fn f(mut x: u32) -> u32 { x += 1; x }",
            "fn f(mut x: u32) -> u32 { x = x + 1; x }",
        ),
        (
            "fn f(x: u64, y: u64) -> u64 { let mut v = x; v -= y; v }",
            "fn f(x: u64, y: u64) -> u64 { let mut v = x; v = v - y; v }",
        ),
        (
            "fn f(mut x: u8, y: u32) -> u8 { x <<= y; x }",
            "fn f(mut x: u8, y: u32) -> u8 { x = x << y; x }",
        ),
        (
            "fn f(mut x: i16) -> i16 { x >>= 3; x }",
            "fn f(mut x: i16) -> i16 { x = x >> 3; x }",
        ),
    ] {
        equivalent(written, expanded);
    }
}

#[test]
fn targets_keep_their_type_through_assignments_and_nested_blocks() {
    for (written, expanded) in [
        (
            "fn f(mut x: u32, y: u32) -> u32 { x += y; x *= y; x }",
            "fn f(mut x: u32, y: u32) -> u32 { x = x + y; x = x * y; x }",
        ),
        (
            "fn f(x: u32, y: u32) -> u32 { let mut v = x; v ^= y; v |= x; v }",
            "fn f(x: u32, y: u32) -> u32 { let mut v = x; v = v ^ y; v = v | x; v }",
        ),
        (
            "fn f(y: u32) -> u32 { let mut v = 0u32; for _ in 0..3u32 { v += y; v ^= y; } v }",
            "fn f(y: u32) -> u32 { let mut v = 0u32; for _ in 0..3u32 { v = v + y; v = v ^ y; } v }",
        ),
        (
            "fn f(p: bool, q: bool) -> bool { let mut all = p; if q { all &= q; } all }",
            "fn f(p: bool, q: bool) -> bool { let mut all = p; if q { all = all & q; } all }",
        ),
    ] {
        equivalent(written, expanded);
    }
}

#[test]
fn expanded_values_take_the_existing_operand_order() {
    for (written, expanded) in [
        ("x ^= y", "x = y ^ x"),
        ("x &= y", "x = y & x"),
        ("x |= y ^ x", "x = x | (x ^ y)"),
    ] {
        equivalent(&function("u32", written), &function("u32", expanded));
    }
}

#[test]
fn accepted_total_helpers_are_proven_operands() {
    let helper = "fn h(x: u32) -> u32 { x & 7u32 }";
    equivalent(
        &format!("{helper} fn f(mut x: u32, y: u32) -> u32 {{ x += h(y); x }}"),
        &format!("{helper} fn f(mut x: u32, y: u32) -> u32 {{ x = x + h(y); x }}"),
    );
}

#[test]
fn places_beyond_a_local_keep_the_compound_operator() {
    for (written, expanded) in [
        (
            "struct S { x: u32 } fn f(mut s: S, y: u32) -> u32 { s.x += y; s.x }",
            "struct S { x: u32 } fn f(mut s: S, y: u32) -> u32 { s.x = s.x + y; s.x }",
        ),
        (
            "fn f(mut a: [u32; 1], y: u32) -> u32 { a[0] += y; a[0] }",
            "fn f(mut a: [u32; 1], y: u32) -> u32 { a[0] = a[0] + y; a[0] }",
        ),
        (
            "fn f(x: &mut u32, y: u32) { *x += y; }",
            "fn f(x: &mut u32, y: u32) { *x = *x + y; }",
        ),
        (
            "static mut X: u32 = 0; fn f(y: u32) { unsafe { X += y; } }",
            "static mut X: u32 = 0; fn f(y: u32) { unsafe { X = X + y; } }",
        ),
    ] {
        separate(written, expanded);
    }
}

#[test]
fn unproven_types_and_operands_keep_the_compound_operator() {
    for ty in ["f32", "f64", "usize", "isize", "char", "String", "Unknown"] {
        separate(&function(ty, "x += y"), &function(ty, "x = x + y"));
    }
    for (written, expanded) in [
        (
            "fn f(mut x: u32, y: &u32) -> u32 { x += y; x }",
            "fn f(mut x: u32, y: &u32) -> u32 { x = x + y; x }",
        ),
        (
            "fn f(mut x: u32) -> u32 { x += read(); x }",
            "fn f(mut x: u32) -> u32 { x = x + read(); x }",
        ),
        (
            "fn f(mut x: u8, y: usize) -> u8 { x <<= y; x }",
            "fn f(mut x: u8, y: usize) -> u8 { x = x << y; x }",
        ),
    ] {
        separate(written, expanded);
    }
}

#[test]
fn observed_assignments_keep_their_source() {
    for source in [
        "#[observe] fn f(mut x: u32, y: u32) -> u32 { x += y; x }",
        "fn f(mut x: u32, y: u32) -> u32 { #[observe] { x += y; } x }",
        "fn f(mut x: u32, y: u32) { unknown!(x += y); }",
    ] {
        separate(source, &source.replace("x += y", "x = x + y"));
    }
}

#[test]
fn function_views_expand_proven_targets_and_lexical_bodies_keep_unknown_types() {
    let file = parse(
        "fn first(mut x: u32, y: u32) -> u32 { x *= y; x } fn second(mut a: u32, b: u32) -> u32 { a = a * b; a }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "fn first(mut x: Unknown, y: Unknown) -> Unknown { x *= y; x } fn second(mut x: Unknown, y: Unknown) -> Unknown { x = x * y; x }",
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
