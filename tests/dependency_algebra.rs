//! Primitive algebra through passing and failing public comparison.

#[path = "support/dependency.rs"]
mod support;

#[path = "support/same.rs"]
mod same;

use same::same;

fn form(source: &str) -> syn_canon::CanonicalForm {
    syn_canon::canonicalize(syn::parse_str(source).unwrap())
}

fn failing(source: &str) -> syn_canon::CanonicalForm {
    syn_canon::canonicalize_failing(syn::parse_str(source).unwrap())
}

fn separate(left: &str, right: &str) {
    assert_ne!(form(left), form(right), "{left}\n{right}");
    assert_ne!(failing(left), failing(right));
}

const TYPES: [&str; 11] = [
    "bool", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
];

#[test]
fn algebra_bitwise_commutation() {
    for ty in TYPES {
        for op in ["&", "|", "^"] {
            same(
                &format!("fn f(a: {ty}, b: {ty}) -> {ty} {{ a {op} b }}"),
                &format!("fn g(x: {ty}, y: {ty}) -> {ty} {{ y {op} x }}"),
            );
        }
    }
}

#[test]
fn algebra_bitwise_association() {
    for ty in TYPES {
        for op in ["&", "|", "^"] {
            same(
                &format!("fn f(a: {ty}, b: {ty}, c: {ty}) -> {ty} {{ (a {op} b) {op} c }}"),
                &format!("fn g(x: {ty}, y: {ty}, z: {ty}) -> {ty} {{ z {op} (y {op} x) }}"),
            );
        }
    }
    same(
        "fn f(x: u32) -> u32 { (x ^ 1u32) ^ 2u32 }",
        "fn f(x: u32) -> u32 { x ^ (1u32 ^ 2u32) }",
    );
}

#[test]
fn algebra_comparison_direction() {
    for ty in TYPES {
        for (forward, reverse) in [(">", "<"), (">=", "<=")] {
            same(
                &format!("fn f(a: {ty}, b: {ty}) -> bool {{ a {forward} b }}"),
                &format!("fn g(x: {ty}, y: {ty}) -> bool {{ y {reverse} x }}"),
            );
        }
    }
}

#[test]
fn algebra_equality_symmetry() {
    for ty in TYPES {
        for op in ["==", "!="] {
            same(
                &format!("fn f(a: {ty}, b: {ty}) -> bool {{ a {op} b }}"),
                &format!("fn g(x: {ty}, y: {ty}) -> bool {{ y {op} x }}"),
            );
        }
    }
    same(
        "fn f(a: u32) -> bool { a == 0u32 }",
        "fn g(x: u32) -> bool { 0u32 == x }",
    );
}

#[test]
fn algebra_in_branch_conditions() {
    for (forward, reverse) in [(">", "<"), (">=", "<=")] {
        same(
            &format!(
                "fn f(a: u32, r: u32) -> u32 {{ if a {forward} 7u32 {{ 0u32 }} else {{ r }} }}"
            ),
            &format!(
                "fn g(x: u32, s: u32) -> u32 {{ if 7u32 {reverse} x {{ 0u32 }} else {{ s }} }}"
            ),
        );
    }
    for (left, right) in [
        (
            "fn f(a: u32, b: u32, r: u32) -> u32 { if (a & b) < 7u32 { 0u32 } else { r } }",
            "fn g(x: u32, y: u32, s: u32) -> u32 { if (y & x) < 7u32 { 0u32 } else { s } }",
        ),
        (
            "fn f(a: bool, b: bool) -> u32 { if a & b { 0u32 } else { 1u32 } }",
            "fn g(x: bool, y: bool) -> u32 { if y & x { 0u32 } else { 1u32 } }",
        ),
        (
            "fn f(a: u32, b: u32) -> u32 { if a > b { return 1u32; } 0u32 }",
            "fn g(x: u32, y: u32) -> u32 { if y < x { return 1u32; } 0u32 }",
        ),
        (
            "fn f(a: u32, r: u32) -> u32 { if a > 7u32 { 0u32 } else if a > 3u32 { 1u32 } else { r } }",
            "fn g(x: u32, s: u32) -> u32 { if 7u32 < x { 0u32 } else if 3u32 < x { 1u32 } else { s } }",
        ),
        (
            "fn f(a: u32, b: u32) -> u32 { if a > b { #[cfg(test)] let _z = 1u32; 0u32 } else { 1u32 } }",
            "fn g(x: u32, y: u32) -> u32 { if y < x { #[cfg(test)] let _z = 1u32; 0u32 } else { 1u32 } }",
        ),
    ] {
        same(left, right);
    }
}

#[test]
fn opposite_branches_over_a_negated_comparison() {
    for (test, negated) in [("<", ">="), ("<=", ">"), ("==", "!=")] {
        same(
            &format!("fn f(a: u32, r: u32) -> u32 {{ if a {test} 7u32 {{ 0u32 }} else {{ r }} }}"),
            &format!(
                "fn g(x: u32, s: u32) -> u32 {{ if x {negated} 7u32 {{ s }} else {{ 0u32 }} }}"
            ),
        );
    }
    same(
        "fn f(a: u32, b: u32) -> u32 { let x = a & 1u32; let y = b & 1u32; if !(x <= y) { println!(\"{x}\"); } else { println!(\"{y}\"); } 0u32 }",
        "fn g(a: u32, b: u32) -> u32 { let y = b & 1u32; let x = a & 1u32; if y < x { println!(\"{x}\"); } else { println!(\"{y}\"); } 0u32 }",
    );
}

#[test]
fn negated_comparisons_take_their_complement() {
    for ty in TYPES {
        for (negated, complement) in [
            ("!(a < b)", "y <= x"),
            ("!(a <= b)", "y < x"),
            ("!(a == b)", "x != y"),
            ("!(a != b)", "x == y"),
        ] {
            same(
                &format!("fn f(a: {ty}, b: {ty}) -> bool {{ {negated} }}"),
                &format!("fn g(x: {ty}, y: {ty}) -> bool {{ {complement} }}"),
            );
        }
    }
}

#[test]
fn algebra_bitwise_idempotence() {
    for ty in TYPES {
        for op in ["&", "|"] {
            same(
                &format!("fn f(a: {ty}) -> {ty} {{ a {op} a }}"),
                &format!("fn g(x: {ty}) -> {ty} {{ x }}"),
            );
        }
    }
}

#[test]
fn algebra_original_fixtures_and_inherited_negatives() {
    for case in support::cases() {
        if case.family == "algebra"
            && [
                "A01_commute_bitwise",
                "A02_associate_bitwise",
                "A03_reverse_comparison",
                "A12_bitwise_idempotence",
            ]
            .contains(&case.id.as_str())
        {
            same(&case.a, &case.b);
        } else if case.expected != "same" {
            separate(&case.a, &case.b);
        }
    }
}

#[test]
fn algebra_effects_and_unstable_reads_remain_ordered() {
    for (left, right) in [
        ("left() & right()", "right() & left()"),
        ("read() | read()", "read()"),
        ("*a & *a", "*a"),
        ("a.get() | a.get()", "a.get()"),
        (
            "(left() ^ right()) ^ third()",
            "left() ^ (right() ^ third())",
        ),
        ("left() > right()", "right() < left()"),
    ] {
        separate(
            &format!("fn f(a: &u32) -> u32 {{ {left} }}"),
            &format!("fn f(a: &u32) -> u32 {{ {right} }}"),
        );
    }
}

#[test]
fn algebra_overloads_and_inference_remain_opaque() {
    for ty in ["Custom", "T", "f32", "usize"] {
        for (left, right) in [
            ("a & b", "b & a"),
            ("(a ^ b) ^ c", "a ^ (b ^ c)"),
            ("a > b", "b < a"),
            ("a | a", "a"),
        ] {
            separate(
                &format!("fn f<T>(a: {ty}, b: {ty}, c: {ty}) {{ {left} }}"),
                &format!("fn f<T>(a: {ty}, b: {ty}, c: {ty}) {{ {right} }}"),
            );
        }
    }
    separate("fn f() { 200 & 100 }", "fn f() { 100 & 200 }");
    separate(
        "fn f(a: u8, b: u16) { a & b }",
        "fn f(a: u8, b: u16) { b & a }",
    );
    separate(
        "type u32 = Custom; fn f(a: u32, b: u32) { a > b }",
        "type u32 = Custom; fn f(a: u32, b: u32) { b < a }",
    );
}

#[test]
fn algebra_storage_boundaries_preserve_expression_results() {
    separate(
        "fn f(a: u32) { let result = &(a & a); consume(result, &a); }",
        "fn f(a: u32) { let result = &a; consume(result, &a); }",
    );
    separate(
        "fn f(a: u32) { let x = a & a; let y = a; consume(&x, &y); }",
        "fn f(a: u32) { let x = a; consume(&x, &x); }",
    );
    separate(
        "fn f() { let mut a = 1u32; let x = a & a; a = 2u32; (x, a) }",
        "fn f() { let mut a = 1u32; let x = a & a; a = 2u32; (a, x) }",
    );
}

#[test]
fn algebra_observers_and_contextual_uncertainty_preserve_syntax() {
    for prefix in ["#[observe]", "#[cfg(feature = \"unknown\")]"] {
        separate(
            &format!("{prefix} fn f(a: u32, b: u32) {{ a & b }}"),
            &format!("{prefix} fn f(a: u32, b: u32) {{ b & a }}"),
        );
    }
    separate(
        "fn f(a: u32, b: u32) { observe!(); a & b }",
        "fn f(a: u32, b: u32) { observe!(); b & a }",
    );
    separate(
        "use unknown::*; fn f(a: u32, b: u32) { a & b }",
        "use unknown::*; fn f(a: u32, b: u32) { b & a }",
    );
    separate(
        "fn f(a: u32, b: u32) { stringify!(a & b) }",
        "fn f(a: u32, b: u32) { stringify!(b & a) }",
    );
}

#[test]
fn algebra_operator_kinds_and_output_positions_remain_distinct() {
    for (left, right) in [
        ("a & b", "a | b"),
        ("a ^ a", "a"),
        ("a > b", "a >= b"),
        ("a == b", "a != b"),
        ("a > b", "a < b"),
        ("(a & b, a)", "(a, b & a)"),
        ("(a & b) | c", "a & (b | c)"),
        ("(a + b) + c", "a + (b + c)"),
    ] {
        separate(
            &format!("fn f(a: u32, b: u32, c: u32) {{ {left} }}"),
            &format!("fn f(a: u32, b: u32, c: u32) {{ {right} }}"),
        );
    }
}

#[test]
fn algebra_commutes_an_operand_chosen_by_scalar_branches() {
    same(
        "fn f(a: u32, p: bool, b: u32) -> u32 { (if p { a } else { 1u32 }) | b }",
        "fn g(x: u32, q: bool, y: u32) -> u32 { y | (if q { x } else { 1u32 }) }",
    );
    same(
        "fn f(a: u32, p: bool, b: u32) -> u32 { let t = if p { a } else { 1u32 }; t | b }",
        "fn g(x: u32, q: bool, y: u32) -> u32 { let t = if q { x } else { 1u32 }; y | t }",
    );
}

#[test]
fn a_binding_chosen_by_scalar_branches_inlines_like_other_temporaries() {
    same(
        "fn f(a: u32, p: bool, b: u32) -> u32 { let t = if p { a } else { 1u32 }; t | b }",
        "fn g(x: u32, q: bool, y: u32) -> u32 { (if q { x } else { 1u32 }) | y }",
    );
}

#[test]
fn independent_bindings_reorder_when_one_is_chosen_by_scalar_branches() {
    same(
        "fn f(a: u32, p: bool, b: u32) -> u32 { let t = if p { a } else { 1u32 }; let u = b & 3u32; t | u }",
        "fn g(x: u32, q: bool, y: u32) -> u32 { let u = y & 3u32; let t = if q { x } else { 1u32 }; t | u }",
    );
}

#[test]
fn branches_that_are_not_total_scalars_keep_their_order() {
    for (left, right) in [
        (
            "(if p { observe(); a } else { 1u32 }) | b",
            "b | (if p { observe(); a } else { 1u32 })",
        ),
        (
            "(if check() { a } else { 1u32 }) | b",
            "b | (if check() { a } else { 1u32 })",
        ),
        ("(if p { a } else { b }) | b", "(if p { b } else { a }) | b"),
        (
            "(if p { a } else { 1u32 }) | b",
            "(if !p { a } else { 1u32 }) | b",
        ),
        (
            "(if p { a } else { 1u64 as u32 }) | b",
            "b | (if p { a } else { 1u64 as u32 })",
        ),
    ] {
        separate(
            &format!("fn f(a: u32, p: bool, b: u32) -> u32 {{ {left} }}"),
            &format!("fn f(a: u32, p: bool, b: u32) -> u32 {{ {right} }}"),
        );
    }
    same(
        "fn f(a: u32, p: bool, b: u32) -> u32 { (if !p { 1u32 } else { a }) | b }",
        "fn f(a: u32, p: bool, b: u32) -> u32 { b | (if p { a } else { 1u32 }) }",
    );
}

#[test]
fn unsuffixed_literal_arithmetic_folds_beside_a_typed_operand() {
    same(
        "fn f(a: u32) -> u32 { a & (2 * 8) }",
        "fn g(x: u32) -> u32 { x & 16 }",
    );
    same("fn f() -> u32 { 2 * 8 }", "fn g() -> u32 { 16 }");
    same(
        "fn f() -> [u8; 2 + 3] { [0; 2 + 3] }",
        "fn f() -> [u8; 5] { [0; 5] }",
    );
    same(
        "fn f(a: u32) -> u32 { a % (7 % 4) }",
        "fn g(x: u32) -> u32 { x % 3 }",
    );
    same(
        "fn f(a: u32) -> u32 { a & (5 & 3) }",
        "fn g(x: u32) -> u32 { x & 1 }",
    );
    same(
        "fn f(a: u32) -> u32 { a >> (16 >> 2) }",
        "fn g(x: u32) -> u32 { x >> 4 }",
    );
}

#[test]
fn unsuffixed_literal_arithmetic_keeps_values_some_integer_type_reads_differently() {
    for (left, right) in [
        ("a & (100 + 28)", "a & 128"),
        ("a & (64 << 1)", "a & 128"),
        ("a & (1 << 8)", "a & 256"),
        ("a & !0", "a & 255"),
        ("a & (2 - 8)", "a & -6"),
        ("a & (7 / 0)", "a & 0"),
    ] {
        separate(
            &format!("fn f(a: i64) -> i64 {{ {left} }}"),
            &format!("fn f(a: i64) -> i64 {{ {right} }}"),
        );
    }
    same(
        "fn f(a: u32) -> u32 { a & ((3 << 4) | 5 ^ 1) }",
        "fn f(a: u32) -> u32 { a & 52 }",
    );
}
