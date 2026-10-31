//! Integer type annotations on `let` bindings whose initializer is an integer literal.

#[path = "support/view.rs"]
mod view;

use syn_canon::ContextKind;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(annotated: &str, suffixed: &str) {
    let left = syn_canon::canonicalize(parse(annotated));
    let right = syn_canon::canonicalize(parse(suffixed));
    assert_eq!(left, right, "{annotated}\n{suffixed}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{annotated}");
    assert_eq!(left.body_units(), right.body_units(), "{annotated}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(annotated)),
        syn_canon::canonicalize_failing(parse(suffixed)),
        "{annotated}\n{suffixed}"
    );
}

fn separate(annotated: &str, suffixed: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(annotated)),
            canonicalize(parse(suffixed)),
            "{annotated}\n{suffixed}"
        );
    }
}

#[test]
fn annotated_literals_match_suffixed_literals_in_every_integer_type() {
    for (ty, max) in [
        ("u8", "255"),
        ("u16", "65535"),
        ("u32", "4294967295"),
        ("u64", "18446744073709551615"),
        ("u128", "340282366920938463463374607431768211455"),
        ("i8", "127"),
        ("i16", "32767"),
        ("i32", "2147483647"),
        ("i64", "9223372036854775807"),
        ("i128", "170141183460469231731687303715884105727"),
    ] {
        for value in ["0", "5", max] {
            equivalent(
                &format!("fn f() -> {ty} {{ let v: {ty} = {value}; v }}"),
                &format!("fn f() -> {ty} {{ let v = {value}{ty}; v }}"),
            );
        }
    }
}

#[test]
fn spellings_negations_and_closed_values_take_the_suffixed_value() {
    for (annotated, suffixed) in [
        (
            "fn f() -> u32 { let v: u32 = 0x10; v }",
            "fn f() -> u32 { let v = 16u32; v }",
        ),
        (
            "fn f() -> u32 { let v: u32 = 1_000; v }",
            "fn f() -> u32 { let v = 1000u32; v }",
        ),
        (
            "fn f() -> i8 { let v: i8 = -128; v }",
            "fn f() -> i8 { let v = -128i8; v }",
        ),
        (
            "fn f() -> u16 { let v: u16 = 2 + 3; v }",
            "fn f() -> u16 { let v = 5u16; v }",
        ),
        (
            "fn f() -> u64 { let v: u64 = 7u64; v }",
            "fn f() -> u64 { let v = 7u64; v }",
        ),
    ] {
        equivalent(annotated, suffixed);
    }
}

#[test]
fn mutable_bindings_and_aliases_take_the_suffixed_value() {
    for (annotated, suffixed) in [
        (
            "fn f(x: u32) -> u32 { let mut v: u32 = 0; v += x; v }",
            "fn f(x: u32) -> u32 { let mut v = 0u32; v += x; v }",
        ),
        (
            "type Count = u32; fn f() -> Count { let v: Count = 5; v }",
            "type Count = u32; fn f() -> Count { let v = 5u32; v }",
        ),
    ] {
        equivalent(annotated, suffixed);
    }
}

#[test]
fn other_patterns_types_and_initializers_keep_the_annotation() {
    for (annotated, suffixed) in [
        (
            "fn f() -> usize { let v: usize = 5; v }",
            "fn f() -> usize { let v = 5usize; v }",
        ),
        (
            "fn f() -> f32 { let v: f32 = 5.0; v }",
            "fn f() -> f32 { let v = 5.0f32; v }",
        ),
        (
            "fn f() -> u8 { let v: u8 = 256; v }",
            "fn f() -> u8 { let v = 256u8; v }",
        ),
        (
            "fn f() -> u32 { let (a, b): (u32, u32) = (1, 2); a ^ b }",
            "fn f() -> u32 { let (a, b) = (1u32, 2u32); a ^ b }",
        ),
        (
            "fn f() -> u32 { let ref v: u32 = 5; *v }",
            "fn f() -> u32 { let ref v = 5u32; *v }",
        ),
        (
            "struct Wide(u64); type u32 = Wide; fn f() { let v: u32 = 5; }",
            "struct Wide(u64); type u32 = Wide; fn f() { let v = 5u32; }",
        ),
        (
            "use other::*; fn f() -> u32 { let v: u32 = 5; v }",
            "use other::*; fn f() -> u32 { let v = 5u32; v }",
        ),
    ] {
        separate(annotated, suffixed);
    }
}

#[test]
fn observed_bindings_keep_their_source() {
    for source in [
        "#[observe] fn f() -> u32 { let v: u32 = 5; v }",
        "fn f() -> u32 { #[observe] let v: u32 = 5; v }",
        "fn f() { unknown!(let v: u32 = 5;); }",
    ] {
        separate(source, &source.replace("let v: u32 = 5", "let v = 5u32"));
    }
}

#[test]
fn function_views_drop_proven_annotations_and_lexical_bodies_keep_unknown_types() {
    let file =
        parse("fn first() -> u32 { let v: u32 = 5; v } fn second() -> u32 { let w = 5u32; w }");
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "fn first() -> Count { let v: Count = 5; v } fn second() -> Count { let v = 5Count; v }",
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
