//! Identity and absorbing operands over proven fixed-width integers and booleans.

#[path = "support/view.rs"]
mod view;

use syn_canon::ContextKind;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, reduced: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(reduced));
    assert_eq!(left, right, "{written}\n{reduced}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(reduced)),
        "{written}\n{reduced}"
    );
}

fn separate(written: &str, reduced: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(reduced)),
            "{written}\n{reduced}"
        );
    }
}

fn function(ty: &str, body: &str) -> String {
    format!("fn f(x: {ty}, y: {ty}) -> {ty} {{ {body} }}")
}

const INTEGERS: [(&str, &str); 10] = [
    ("u8", "255u8"),
    ("u16", "65535u16"),
    ("u32", "4294967295u32"),
    ("u64", "18446744073709551615u64"),
    ("u128", "340282366920938463463374607431768211455u128"),
    ("i8", "-1i8"),
    ("i16", "-1i16"),
    ("i32", "-1i32"),
    ("i64", "-1i64"),
    ("i128", "-1i128"),
];

#[test]
fn identity_operands_leave_the_other_operand() {
    for (ty, max) in INTEGERS {
        for written in [
            "x + 0".to_owned(),
            "0 + x".to_owned(),
            "x - 0".to_owned(),
            "x * 1".to_owned(),
            "1 * x".to_owned(),
            "x / 1".to_owned(),
            "x | 0".to_owned(),
            "0 | x".to_owned(),
            "x << 0".to_owned(),
            "x >> 0u32".to_owned(),
            format!("x & {max}"),
            format!("{max} & x"),
            "x.wrapping_add(0)".to_owned(),
            "x.saturating_add(0)".to_owned(),
            "x.wrapping_mul(1)".to_owned(),
            "x.saturating_mul(1)".to_owned(),
        ] {
            equivalent(&function(ty, &written), &function(ty, "x"));
        }
    }
}

#[test]
fn absorbing_operands_give_the_typed_constant() {
    for (ty, max) in INTEGERS {
        for (written, reduced) in [
            ("x & 0".to_owned(), format!("0{ty}")),
            ("x * 0".to_owned(), format!("0{ty}")),
            ("0 * x".to_owned(), format!("0{ty}")),
            ("x - x".to_owned(), format!("0{ty}")),
            (format!("x | {max}"), max.to_owned()),
        ] {
            equivalent(&function(ty, &written), &function(ty, &reduced));
        }
    }
}

#[test]
fn boolean_bitwise_literals_reduce_like_masks() {
    for (written, reduced) in [
        ("p & true", "p"),
        ("p | false", "p"),
        ("p & false", "false"),
        ("p | true", "true"),
    ] {
        equivalent(
            &format!("fn f(p: bool) -> bool {{ {written} }}"),
            &format!("fn f(p: bool) -> bool {{ {reduced} }}"),
        );
    }
}

#[test]
fn identities_reach_through_nested_and_chained_operands() {
    for (written, reduced) in [
        ("(x ^ y) + 0", "y ^ x"),
        ("(x | 0) * (1 * y)", "x * y"),
        ("x | 0 | y", "y | x"),
        ("(x & 4294967295u32) & y", "x & y"),
        ("(x - x) ^ y", "y"),
        ("(x * 0) | y", "y"),
    ] {
        equivalent(&function("u32", written), &function("u32", reduced));
    }
}

#[test]
fn effects_partial_values_and_other_constants_stay() {
    for (written, reduced) in [
        ("read() * 0", "0u32"),
        ("read() & 0", "0u32"),
        ("read() - read()", "0u32"),
        ("x / 0", "x"),
        ("0 / x", "0u32"),
        ("x * 2", "x"),
        ("x - 1 + 1", "x"),
        ("x.abs_diff(0)", "x"),
        ("x.wrapping_sub(0)", "x"),
    ] {
        separate(&function("u32", written), &function("u32", reduced));
    }
    for ty in ["usize", "f32", "Unknown"] {
        separate(&function(ty, "x + 0"), &function(ty, "x"));
    }
    separate(
        "fn f(x: i32) -> i32 { x & 4294967295u32 as i32 }",
        "fn f(x: i32) -> i32 { x }",
    );
}

#[test]
fn borrowed_storage_keeps_its_expression() {
    separate(
        "fn f(x: u32) -> &'static u32 { &(x | 0) }",
        "fn f(x: u32) -> &'static u32 { &x }",
    );
}

#[test]
fn observed_identities_keep_their_source() {
    for source in [
        "#[observe] fn f(x: u32) -> u32 { x + 0 }",
        "fn f(x: u32) -> u32 { #[observe] (x + 0) }",
        "fn f(x: u32) { unknown!(x + 0); }",
    ] {
        separate(source, &source.replace("x + 0", "x"));
    }
}

#[test]
fn function_views_reduce_proven_values_and_lexical_bodies_keep_unknown_types() {
    let file = parse(
        "fn first(x: u32, y: u32) -> u32 { (x | 0) ^ (y * 1) } fn second(a: u32, b: u32) -> u32 { a ^ b }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file =
        parse("fn first(x: Unknown) -> Unknown { x + 0 } fn second(x: Unknown) -> Unknown { x }");
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
