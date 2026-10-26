//! Operand order of native integer `wrapping_add` over resolved total scalar values.

#[path = "support/view.rs"]
mod view;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, reordered: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(reordered));
    assert_eq!(left, right, "{written}\n{reordered}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens());
    assert_eq!(left.body_units(), right.body_units());
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(reordered)),
        "{written}\n{reordered}",
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

fn function(ty: &str, body: &str) -> String {
    format!("fn f(x: {ty}, y: {ty}, z: {ty}) -> {ty} {{ {body} }}")
}

#[test]
fn operands_share_one_order_in_every_fixed_width_integer_domain() {
    for ty in [
        "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
    ] {
        equivalent(
            &function(ty, "x.wrapping_add(y)"),
            &function(ty, "y.wrapping_add(x)"),
        );
    }
}

#[test]
fn literal_operands_keep_a_typed_receiver() {
    equivalent(
        &function("u32", "x.wrapping_add(7u32)"),
        &function("u32", "7u32.wrapping_add(x)"),
    );
    equivalent(
        "fn f(x: i32) -> i32 { x.wrapping_add(-1i32) }",
        "fn f(x: i32) -> i32 { (-1i32).wrapping_add(x) }",
    );
    separate(
        &function("u32", "x.wrapping_add(7)"),
        &function("u32", "x.wrapping_add(8)"),
    );
}

#[test]
fn total_operands_order_by_normalized_labels() {
    for (written, reordered) in [
        ("x.wrapping_add(y & 7u32)", "(y & 7u32).wrapping_add(x)"),
        ("(x ^ y).wrapping_add(z)", "z.wrapping_add(y ^ x)"),
        (
            "x.wrapping_add(y).wrapping_add(z)",
            "z.wrapping_add(y.wrapping_add(x))",
        ),
        ("x.wrapping_add(y << 1u32)", "(y << 1u32).wrapping_add(x)"),
        (
            "x.wrapping_add(if y < z { y } else { z })",
            "(if y < z { y } else { z }).wrapping_add(x)",
        ),
        ("x.wrapping_add(y ^ y)", "0u32.wrapping_add(x)"),
    ] {
        equivalent(&function("u32", written), &function("u32", reordered));
    }
}

#[test]
fn reordered_values_take_part_in_existing_laws() {
    equivalent(
        &function("u32", "x.wrapping_add(y) ^ y.wrapping_add(x)"),
        &function("u32", "0u32"),
    );
    equivalent(
        "fn f(x: u32, y: u32) -> bool { x.wrapping_add(y) == y.wrapping_add(x) }",
        "fn f(x: u32, y: u32) -> bool { true }",
    );
    equivalent(
        &function("u32", "let s = x.wrapping_add(y); s"),
        &function("u32", "let s = y.wrapping_add(x); s"),
    );
    equivalent(
        &function(
            "u32",
            "let a = x.wrapping_add(1u32); let b = y; a.wrapping_add(b)",
        ),
        &function(
            "u32",
            "let b = y; let a = 1u32.wrapping_add(x); b.wrapping_add(a)",
        ),
    );
}

#[test]
fn associativity_and_other_wrapping_identities_stay_out_of_scope() {
    for (written, other) in [
        (
            "x.wrapping_add(y).wrapping_add(z)",
            "x.wrapping_add(y.wrapping_add(z))",
        ),
        ("x.wrapping_add(0u32)", "x"),
        ("1u32.wrapping_add(2u32)", "3u32"),
        ("x.wrapping_sub(y)", "y.wrapping_sub(x)"),
        ("x.wrapping_mul(y)", "y.wrapping_mul(x)"),
        ("x.saturating_add(y)", "y.saturating_add(x)"),
        ("x.wrapping_add(y)", "x + y"),
    ] {
        separate(&function("u32", written), &function("u32", other));
    }
}

#[test]
fn accepted_total_helpers_order_only_with_proven_arguments() {
    let helper = "fn h(x: u32) -> u32 { x & 7u32 }";
    equivalent(
        &format!("{helper} fn f(x: u32, y: u32) -> u32 {{ h(x).wrapping_add(y) }}"),
        &format!("{helper} fn f(x: u32, y: u32) -> u32 {{ y.wrapping_add(h(x)) }}"),
    );
    separate(
        &format!("{helper} fn f(y: u32) -> u32 {{ h(5).wrapping_add(y) }}"),
        &format!("{helper} fn f(y: u32) -> u32 {{ y.wrapping_add(h(5)) }}"),
    );
}

#[test]
fn ordering_retains_preceding_effects() {
    equivalent(
        &function("u32", "read(); x.wrapping_add(y)"),
        &function("u32", "read(); y.wrapping_add(x)"),
    );
    separate(
        &function("u32", "read(); x.wrapping_add(y)"),
        &function("u32", "y.wrapping_add(x)"),
    );
}

#[test]
fn unsupported_domains_aliases_and_inference_stay_ordered() {
    for ty in ["usize", "isize", "f64", "bool", "char", "Unknown"] {
        separate(
            &format!("fn f(x: {ty}, y: {ty}) {{ x.wrapping_add(y); }}"),
            &format!("fn f(x: {ty}, y: {ty}) {{ y.wrapping_add(x); }}"),
        );
    }
    for written in [
        "fn f(y: u32) { let x = read(); x.wrapping_add(y); }",
        "fn f() { let x = 1; let y = 2; x.wrapping_add(y); }",
        "fn f<T>(x: T, y: T) { x.wrapping_add(y); }",
        "type u32 = Unknown; fn f(x: u32, y: u32) { x.wrapping_add(y); }",
        "use other::*; fn f(x: u32, y: u32) { x.wrapping_add(y); }",
        "type Word = Unknown; fn f(x: Word, y: Word) { x.wrapping_add(y); }",
        "fn f(x: u32, y: i32) { x.wrapping_add(y); }",
    ] {
        separate(
            written,
            &written.replace("x.wrapping_add(y)", "y.wrapping_add(x)"),
        );
    }
    separate(
        "fn f(y: u32) { 7.wrapping_add(y); }",
        "fn f(y: u32) { y.wrapping_add(7); }",
    );
}

#[test]
fn custom_methods_and_qualified_calls_stay_ordered() {
    let custom = "#[derive(Clone, Copy)] struct T(u32);\nimpl T {\n    fn wrapping_add(self, other: T) -> T { T(self.0.wrapping_sub(other.0)) }\n}\n";
    separate(
        &format!("{custom}fn f(x: T, y: T) -> T {{ x.wrapping_add(y) }}"),
        &format!("{custom}fn f(x: T, y: T) -> T {{ y.wrapping_add(x) }}"),
    );
    let tr = "trait W { fn wrapping_add(self, other: Self) -> Self; }\n";
    for (written, other) in [
        ("W::wrapping_add(x, y)", "W::wrapping_add(y, x)"),
        (
            "<u32 as W>::wrapping_add(x, y)",
            "<u32 as W>::wrapping_add(y, x)",
        ),
        ("u32::wrapping_add(x, y)", "u32::wrapping_add(y, x)"),
        ("x.wrapping_add::<u8>(y)", "y.wrapping_add::<u8>(x)"),
        ("x.r#wrapping_add(y)", "y.r#wrapping_add(x)"),
    ] {
        separate(
            &format!("{tr}fn f(x: u32, y: u32) -> u32 {{ {written} }}"),
            &format!("{tr}fn f(x: u32, y: u32) -> u32 {{ {other} }}"),
        );
    }
}

#[test]
fn effectful_partial_and_observed_operands_stay_ordered() {
    for (written, other) in [
        ("read().wrapping_add(x)", "x.wrapping_add(read())"),
        ("a().wrapping_add(b())", "b().wrapping_add(a())"),
        ("(1u32 / x).wrapping_add(y)", "y.wrapping_add(1u32 / x)"),
        ("(x + 1u32).wrapping_add(y)", "y.wrapping_add(x + 1u32)"),
        ("(x << 32u32).wrapping_add(y)", "y.wrapping_add(x << 32u32)"),
        ("x.wrapping_add(y, z)", "y.wrapping_add(x, z)"),
    ] {
        separate(&function("u32", written), &function("u32", other));
    }
    for written in [
        "#[observe] fn f(x: u32, y: u32) -> u32 { x.wrapping_add(y) }",
        "fn f(x: u32, y: u32) -> u32 { #[observe] x.wrapping_add(y) }",
        "fn f(x: u32, y: u32) { unknown!(x.wrapping_add(y)); }",
        "fn f(x: u32, y: u32) { stringify!(x.wrapping_add(y)); }",
    ] {
        separate(
            written,
            &written.replace("x.wrapping_add(y)", "y.wrapping_add(x)"),
        );
    }
}

#[test]
fn borrowed_temporaries_share_the_operand_order() {
    equivalent(
        "fn f(x: u32, y: u32) { consume(&x.wrapping_add(y)); }",
        "fn f(x: u32, y: u32) { consume(&y.wrapping_add(x)); }",
    );
    equivalent(
        "fn f(x: u32, y: u32) -> String { x.wrapping_add(y).to_string() }",
        "fn f(x: u32, y: u32) -> String { y.wrapping_add(x).to_string() }",
    );
}

#[test]
fn borrowed_views_share_the_operand_order_without_expanding_lexical_proofs() {
    let file = parse(
        "fn a(x: u32, y: u32) -> u32 { x.wrapping_add(y) } fn b(p: u32, q: u32) -> u32 { q.wrapping_add(p) }",
    );
    assert_eq!(view::view_of(&file, "a"), view::view_of(&file, "b"));
    let file = parse(
        "fn a(x: Unknown, y: Unknown) { x.wrapping_add(y); } fn b(x: Unknown, y: Unknown) { y.wrapping_add(x); }",
    );
    assert_ne!(view::view_of(&file, "a"), view::view_of(&file, "b"));
}
