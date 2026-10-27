//! Exhaustive literal boolean matches over proven total scalar values.

#[path = "support/view.rs"]
mod view;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(matched: &str, conditional: &str) {
    let left = syn_canon::canonicalize(parse(matched));
    let right = syn_canon::canonicalize(parse(conditional));
    assert_eq!(left, right, "{matched}\n{conditional}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens());
    assert_eq!(left.body_units(), right.body_units());
    assert_ne!(
        syn_canon::canonicalize_failing(parse(matched)),
        syn_canon::canonicalize_failing(parse(conditional)),
        "{matched}\n{conditional}",
    );
}

fn separate(matched: &str, conditional: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(matched)),
            canonicalize(parse(conditional)),
            "{matched}\n{conditional}"
        );
    }
}

fn function(ty: &str, body: &str) -> String {
    format!("fn f(p: bool, q: bool, x: {ty}, y: {ty}) -> {ty} {{ {body} }}")
}

#[test]
fn both_arm_orders_match_the_conditional_in_every_primitive_domain() {
    for ty in [
        "bool", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
    ] {
        for matched in [
            "match p { true => x, false => y }",
            "match p { false => y, true => x }",
            "match p { true => { x } false => { y } }",
            "match p { false => { y }, true => x, }",
        ] {
            equivalent(
                &function(ty, matched),
                &function(ty, "if p { x } else { y }"),
            );
        }
    }
}

#[test]
fn proven_conditions_and_branch_values_reuse_the_conditional_proof() {
    for (matched, conditional) in [
        (
            "match x < y { true => x, false => y }",
            "if x < y { x } else { y }",
        ),
        (
            "match !p { true => x, false => y }",
            "if !p { x } else { y }",
        ),
        (
            "match !p { true => x, false => y }",
            "if p { y } else { x }",
        ),
        (
            "match p && q { false => y, true => x }",
            "if p && q { x } else { y }",
        ),
        (
            "match true { true => x, false => y }",
            "if true { x } else { y }",
        ),
        (
            "match p { true => x ^ x, false => y }",
            "if p { 0u32 } else { y }",
        ),
        (
            "match p { true => x & y, false => y & x }",
            "if p { x & y } else { x & y }",
        ),
        (
            "match p { true => match q { false => y, true => x }, false => y }",
            "if p { if q { x } else { y } } else { y }",
        ),
        (
            "let v = match p { false => y, true => x }; v",
            "let v = if p { x } else { y }; v",
        ),
        (
            "read(); match p { false => y, true => x }",
            "read(); if p { x } else { y }",
        ),
    ] {
        equivalent(&function("u32", matched), &function("u32", conditional));
    }
    for (matched, conditional) in [
        ("match p { true => true, false => false }", "p"),
        ("match p { true => false, false => true }", "!p"),
        (
            "match p { false => q, true => true }",
            "if p { true } else { q }",
        ),
    ] {
        equivalent(&function("bool", matched), &function("bool", conditional));
    }
}

#[test]
fn accepted_total_helpers_select_like_the_conditional() {
    let helper = "fn h(x: u32) -> u32 { x & 7u32 }";
    equivalent(
        &format!(
            "{helper} fn f(p: bool, x: u32) -> u32 {{ match p {{ false => x, true => h(x) }} }}"
        ),
        &format!("{helper} fn f(p: bool, x: u32) -> u32 {{ if p {{ h(x) }} else {{ x }} }}"),
    );
}

#[test]
fn unsupported_patterns_stay_matches() {
    for matched in [
        "match p { true => x, b => y }",
        "match p { false | true => x }",
        "match p { true if q => x, false => y }",
        "match p { true if q => x, true => x, false => y }",
        "match p { true => x, false => y, _ => y }",
        "match p { #[cfg(all())] true => x, false => y }",
        "match p { 0 => x, 1 => y }",
        "match p { true => x, true => y }",
    ] {
        separate(
            &function("u32", matched),
            &function("u32", "if p { x } else { y }"),
        );
    }
}

#[test]
fn unproven_scrutinees_keep_their_temporary_scope() {
    for (matched, conditional) in [
        (
            "match read() { true => x, false => y }",
            "if read() { x } else { y }",
        ),
        (
            "match *r { true => x, false => y }",
            "if *r { x } else { y }",
        ),
        (
            "match cell.borrow().flag { true => x, false => y }",
            "if cell.borrow().flag { x } else { y }",
        ),
        ("match z { true => x, false => y }", "if z { x } else { y }"),
    ] {
        separate(
            &format!("fn f(r: &bool, z: Unknown, x: u32, y: u32) -> u32 {{ {matched} }}"),
            &format!("fn f(r: &bool, z: Unknown, x: u32, y: u32) -> u32 {{ {conditional} }}"),
        );
    }
}

#[test]
fn effectful_partial_and_incompatible_branches_stay_matches() {
    for (matched, conditional) in [
        (
            "match p { true => read(), false => y }",
            "if p { read() } else { y }",
        ),
        (
            "match p { true => x / y, false => y }",
            "if p { x / y } else { y }",
        ),
        (
            "match p { true => x + 1u32, false => y }",
            "if p { x + 1u32 } else { y }",
        ),
        (
            "let v = match p { true => return x, false => y }; v",
            "let v = if p { return x } else { y }; v",
        ),
        ("match p { true => 1, false => 2 }", "if p { 1 } else { 2 }"),
        (
            "match p { true => x, false => 1u8 }",
            "if p { x } else { 1u8 }",
        ),
        (
            "match p { true => { read(); x } false => y }",
            "if p { read(); x } else { y }",
        ),
    ] {
        separate(&function("u32", matched), &function("u32", conditional));
    }
}

#[test]
fn borrowed_and_coerced_branches_stay_matches() {
    for (matched, conditional) in [
        (
            "fn f(p: bool, x: u32, y: u32) -> &'static u32 { match p { true => &1u32, false => &2u32 } }",
            "fn f(p: bool, x: u32, y: u32) -> &'static u32 { if p { &1u32 } else { &2u32 } }",
        ),
        (
            "fn f(p: bool, x: &[u8; 2], y: &[u8]) -> &[u8] { match p { true => x, false => y } }",
            "fn f(p: bool, x: &[u8; 2], y: &[u8]) -> &[u8] { if p { x } else { y } }",
        ),
    ] {
        separate(matched, conditional);
    }
}

#[test]
fn observed_syntax_keeps_the_match() {
    for matched in [
        "#[observe] fn f(p: bool, x: u32, y: u32) -> u32 { match p { true => x, false => y } }",
        "fn f(p: bool, x: u32, y: u32) -> u32 { #[observe] match p { true => x, false => y } }",
        "fn f(p: bool, x: u32, y: u32) { unknown!(match p { true => x, false => y }); }",
        "fn f(p: bool, x: u32, y: u32) -> u32 { match p { true => #[observe] x, false => y } }",
    ] {
        let conditional = matched
            .replace(
                "match p { true => #[observe] x, false => y }",
                "if p { #[observe] x } else { y }",
            )
            .replace("match p { true => x, false => y }", "if p { x } else { y }");
        separate(matched, &conditional);
    }
    separate(
        "fn f(p: bool, x: u32, y: u32) -> u32 { match (#[observe] p) { true => x, false => y } }",
        "fn f(p: bool, x: u32, y: u32) -> u32 { if (#[observe] p) { x } else { y } }",
    );
}

#[test]
fn borrowed_views_share_the_fold_without_expanding_lexical_proofs() {
    let file = parse(
        "fn a(p: bool, x: u32, y: u32) -> u32 { match p { false => y, true => x } } fn b(p: bool, x: u32, y: u32) -> u32 { if p { x } else { y } }",
    );
    assert_eq!(view::view_of(&file, "a"), view::view_of(&file, "b"));
    let file = parse(
        "fn a(p: Unknown, x: u32, y: u32) -> u32 { match p { false => y, true => x } } fn b(p: Unknown, x: u32, y: u32) -> u32 { if p { x } else { y } }",
    );
    assert_ne!(view::view_of(&file, "a"), view::view_of(&file, "b"));
}
