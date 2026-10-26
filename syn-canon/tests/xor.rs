//! XOR parity over resolved total scalar values.

#[path = "support/view.rs"]
mod view;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, reduced: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(reduced));
    assert_eq!(left, right, "{written}\n{reduced}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens());
    assert_eq!(left.body_units(), right.body_units());
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(reduced)),
        "{written}\n{reduced}",
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

#[test]
fn pairs_and_chains_cancel_in_every_existing_primitive_domain() {
    for ty in [
        "bool", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
    ] {
        let zero = if ty == "bool" {
            "false".to_owned()
        } else {
            format!("0{ty}")
        };
        for (written, reduced) in [
            ("x ^ x", zero.as_str()),
            ("x ^ y ^ x", "y"),
            ("x ^ x ^ x", "x"),
            ("x ^ y ^ x ^ y", zero.as_str()),
            ("x ^ y ^ x ^ y ^ x", "x"),
        ] {
            equivalent(
                &format!("fn f(x: {ty}, y: {ty}) -> {ty} {{ {written} }}"),
                &format!("fn f(x: {ty}, y: {ty}) -> {ty} {{ {reduced} }}"),
            );
        }
    }
}

#[test]
fn literal_subsets_and_zero_identities_share_one_result() {
    for (written, reduced) in [
        ("x ^ 0u32", "x"),
        ("0u32 ^ x", "x"),
        ("x ^ 1u32 ^ 2u32 ^ 3u32", "x"),
        ("x ^ 7u32 ^ x", "7u32"),
        ("x ^ 7 ^ x", "x ^ x ^ 7"),
        ("(x ^ 7 ^ x) ^ (x ^ 7 ^ x)", "0u32"),
        ("(x ^ y) ^ (y ^ x)", "0u32"),
        ("x ^ (y ^ (x ^ y))", "0u32"),
        ("x ^ y ^ x ^ 0u32", "y"),
    ] {
        equivalent(
            &format!("fn f(x: u32, y: u32) -> u32 {{ {written} }}"),
            &format!("fn f(x: u32, y: u32) -> u32 {{ {reduced} }}"),
        );
    }
    equivalent(
        "fn f(x: bool) -> bool { x ^ false }",
        "fn f(x: bool) -> bool { x }",
    );
    equivalent(
        "fn f(x: bool) -> bool { x ^ true ^ x }",
        "fn f(x: bool) -> bool { true }",
    );
    for ty in ["i8", "i16", "i32", "i64", "i128"] {
        equivalent(
            &format!("fn f(x: {ty}) -> {ty} {{ x ^ -1{ty} ^ -1{ty} }}"),
            &format!("fn f(x: {ty}) -> {ty} {{ x }}"),
        );
    }
}

#[test]
fn total_expression_identity_uses_normalized_labels() {
    for expression in [
        "x & 7u32",
        "x << 1u32",
        "if x < 7u32 { 1u32 } else { 2u32 }",
    ] {
        equivalent(
            &format!("fn f(x: u32) -> u32 {{ ({expression}) ^ ({expression}) }}"),
            "fn f(x: u32) -> u32 { 0u32 }",
        );
    }
    equivalent(
        "fn f(x: u32, y: u32) -> u32 { (x & y) ^ (y & x) }",
        "fn f(x: u32, y: u32) -> u32 { 0u32 }",
    );
}

#[test]
fn accepted_total_helpers_participate_only_with_proven_arguments() {
    let helper = "fn h(x: u32) -> u32 { x & 7u32 }";
    for expression in [
        "h(x) ^ h(x)",
        "(h(x) & 3u32) ^ (h(x) & 3u32)",
        "h(5u32) ^ h(5u32)",
    ] {
        equivalent(
            &format!("{helper} fn f(x: u32) -> u32 {{ {expression} }}"),
            &format!("{helper} fn f(x: u32) -> u32 {{ 0u32 }}"),
        );
    }
    equivalent(
        "fn h(x: u32) -> u32 { x ^ x } fn f(x: u32) -> u32 { h(x) ^ h(x) }",
        "fn h(x: u32) -> u32 { 0u32 } fn f(x: u32) -> u32 { 0u32 }",
    );
    separate(
        &format!("{helper} fn f() -> u32 {{ h(5) ^ h(5) }}"),
        &format!("{helper} fn f() -> u32 {{ 0u32 }}"),
    );
}

#[test]
fn cancellation_retains_preceding_effects() {
    equivalent(
        "fn f(x: u32) -> u32 { read(); x ^ x }",
        "fn f(x: u32) -> u32 { read(); 0u32 }",
    );
    separate(
        "fn f(x: u32) -> u32 { read(); x ^ x }",
        "fn f(x: u32) -> u32 { 0u32 }",
    );
}

#[test]
fn odd_runs_and_distinct_origins_remain_observable() {
    separate(
        "fn f(x: u32, y: u32) -> u32 { x ^ y }",
        "fn f(x: u32, y: u32) -> u32 { 0u32 }",
    );
    separate(
        "fn f(x: u32) -> u32 { x ^ x ^ x }",
        "fn f(x: u32) -> u32 { 0u32 }",
    );
    separate(
        "fn f(x: u32) -> u32 { let old = x; let x: u32 = read(); x ^ old }",
        "fn f(x: u32) -> u32 { let old = x; let x: u32 = read(); 0u32 }",
    );
}

#[test]
fn unsupported_domains_and_inference_keep_their_constraints() {
    for ty in ["usize", "isize", "f64", "char", "String", "Unknown"] {
        separate(
            &format!("fn f(x: {ty}) {{ x ^ x; }}"),
            &format!("fn f(x: {ty}) {{}}"),
        );
    }
    for written in [
        "fn f() { let x = read(); x ^ x; }",
        "fn f() { let x: u32 = read(); x ^ x; }",
        "fn f() { let x = 1; x ^ x; }",
        "fn f<T>(x: T) { x ^ x; }",
        "type u32 = Unknown; fn f(x: u32) { x ^ x; }",
    ] {
        separate(written, &written.replace("x ^ x", "0u32"));
    }
}

#[test]
fn effectful_partial_and_observed_operands_do_not_cancel() {
    for expression in ["read()", "x + 1u32", "1u32 / x", "x << 32u32"] {
        separate(
            &format!("fn f(x: u32) -> u32 {{ ({expression}) ^ ({expression}) }}"),
            "fn f(x: u32) -> u32 { 0u32 }",
        );
    }
    for written in [
        "#[observe] fn f(x: u32) -> u32 { x ^ x }",
        "fn f(x: u32) -> u32 { #[observe] (x ^ x) }",
        "fn f(x: u32) { unknown!(x ^ x); }",
        "fn f(x: u32) { stringify!(x ^ x); }",
        "fn h(x: u32) -> u32 { x & 7u32 } fn f(x: u32) -> u32 { h(#[observe] x) ^ h(#[observe] x) }",
    ] {
        let reduced = written
            .replace("x ^ x", "0u32")
            .replace("h(#[observe] x) ^ h(#[observe] x)", "0u32");
        separate(written, &reduced);
    }
}

#[test]
fn storage_boundaries_preserve_temporary_identity() {
    for written in [
        "fn f(x: u32) { consume(&(x ^ x)); }",
        "fn f(x: u32) -> *const u32 { &(x ^ x) as *const u32 }",
        "fn f(x: u32) -> String { (x ^ x).to_string() }",
        "fn f(x: u32) { consume(&(x ^ 0u32)); }",
        "fn f() -> *const u32 { let x = 3u32; &(x ^ x) as *const u32 }",
        "fn f(y: u32) -> *const u32 { let x = y & 7u32; &(x ^ x) as *const u32 }",
    ] {
        let reduced = written.replace("x ^ x", "0u32").replace("x ^ 0u32", "x");
        separate(written, &reduced);
    }
}

#[test]
fn borrowed_views_share_cancellation_without_expanding_lexical_proofs() {
    let file = parse("fn a(x: u32) -> u32 { x ^ x } fn b(y: u32) -> u32 { 0u32 }");
    assert_eq!(view::view_of(&file, "a"), view::view_of(&file, "b"));
    let file = parse("fn a(x: Unknown) { x ^ x; } fn b(y: Unknown) { 0u32; }");
    let mut forms = Vec::new();
    syn_canon::contexts(&file, &mut |context, form| {
        if context.kind == syn_canon::ContextKind::FunctionBody {
            forms.push(form);
        }
    });
    assert_ne!(forms[0], forms[1]);
}
