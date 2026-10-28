//! Adjacent arms with equal bodies join into or-patterns, and `let`-`else` reads as `match`.

#[path = "support/view.rs"]
mod view;

use syn_canon::ContextKind;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, joined: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(joined));
    assert_eq!(left, right, "{written}\n{joined}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(joined)),
        "{written}\n{joined}"
    );
}

fn separate(written: &str, joined: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(joined)),
            "{written}\n{joined}"
        );
    }
}

fn function(body: &str) -> String {
    format!(
        "enum E {{ A, B, C }} fn f(p: bool, x: u32, y: u32, e: E, t: (u32, u32)) -> u32 {{ {body} }}"
    )
}

#[test]
fn adjacent_arms_with_equal_bodies_join() {
    for (split, joined) in [
        (
            "match x { 0 => y, 1 => y, _ => x }",
            "match x { 0 | 1 => y, _ => x }",
        ),
        (
            "match x { 1 => y, 0 => y, _ => x }",
            "match x { 0 | 1 => y, _ => x }",
        ),
        (
            "match x { 0 => y, 1 => y, 2 => y, _ => x }",
            "match x { 0 | 1 | 2 => y, _ => x }",
        ),
        (
            "match x { 0 | 1 => y, 2 => y, _ => x }",
            "match x { 0 | 1 | 2 => y, _ => x }",
        ),
        (
            "match e { E::A => x, E::B => x, E::C => y }",
            "match e { E::A | E::B => x, E::C => y }",
        ),
        (
            "match t { (0, _) => y, (_, 0) => y, _ => x }",
            "match t { (0, _) | (_, 0) => y, _ => x }",
        ),
        (
            "match x { 0 => { mark(); y } 1 => { mark(); y } _ => x }",
            "match x { 0 | 1 => { mark(); y } _ => x }",
        ),
        (
            "match x { 0..=3 => y, 7 => y, _ => x }",
            "match x { 0..=3 | 7 => y, _ => x }",
        ),
    ] {
        equivalent(&function(split), &function(joined));
    }
    for (split, joined) in [
        (
            "fn f(s: &[u32], y: u32) -> u32 { match s { [0, ..] => y, [] => y, _ => 1 } }",
            "fn f(s: &[u32], y: u32) -> u32 { match s { [0, ..] | [] => y, _ => 1 } }",
        ),
        (
            "fn f(r: &u32, y: u32) -> u32 { match r { &(0 | 1) => y, &2 => y, _ => 3 } }",
            "fn f(r: &u32, y: u32) -> u32 { match r { &(0 | 1) | &2 => y, _ => 3 } }",
        ),
        (
            "struct P { a: u32, b: u32 } fn f(p: P, y: u32) -> u32 { match p { P { a: 0, .. } => y, P { a: 1, b: 0 } => y, _ => 2 } }",
            "struct P { a: u32, b: u32 } fn f(p: P, y: u32) -> u32 { match p { P { a: 0, .. } | P { a: 1, b: 0 } => y, _ => 2 } }",
        ),
    ] {
        equivalent(split, joined);
    }
    separate(
        "struct P { a: u32, b: u32 } fn f(p: P, y: u32) -> u32 { match p { P { a, b: 0 } => a, P { a, b: 1 } => a, _ => y } }",
        "struct P { a: u32, b: u32 } fn f(p: P, y: u32) -> u32 { match p { P { a, b: 0 } | P { a, b: 1 } => a, _ => y } }",
    );
}

#[test]
fn other_arms_stay_apart() {
    for (split, joined) in [
        (
            "match t { (a, 0) => a, (a, 1) => a, _ => x }",
            "match t { (a, 0) | (a, 1) => a, _ => x }",
        ),
        (
            "match x { 0 if p => y, 1 => y, _ => x }",
            "match x { 0 | 1 => y, _ => x }",
        ),
        (
            "match x { 0 => y, 2..=5 => x, 1 => y, _ => x }",
            "match x { 0 | 1 => y, 2..=5 => x, _ => x }",
        ),
        (
            "match x { 0 => y, 1 => x, _ => x }",
            "match x { 0 | 1 => y, _ => x }",
        ),
    ] {
        separate(&function(split), &function(joined));
    }
    separate(
        "fn f(x: u32, y: u32) -> u32 { match x { #[observe] 0 => y, 1 => y, _ => x } }",
        "fn f(x: u32, y: u32) -> u32 { match x { #[observe] 0 | 1 => y, _ => x } }",
    );
    separate(
        "#[observe] fn f(x: u32, y: u32) -> u32 { match x { 0 => y, 1 => y, _ => x } }",
        "#[observe] fn f(x: u32, y: u32) -> u32 { match x { 0 | 1 => y, _ => x } }",
    );
}

fn optional(body: &str) -> String {
    format!("fn f(o: Option<u32>, q: Option<u32>, y: u32) -> u32 {{ {body} }}")
}

#[test]
fn let_else_over_a_place_reads_as_match() {
    for (written, matched) in [
        (
            "let Some(v) = o else { return y; }; v",
            "match o { Some(v) => { v } _ => { return y; } }",
        ),
        (
            "let Some(v) = o else { return y; }; mark(v); y",
            "match o { Some(v) => { mark(v); y } _ => { return y; } }",
        ),
        (
            "let Some(a) = o else { return 0; }; let Some(b) = q else { return 1; }; mark(a, b); y",
            "match o { Some(a) => { match q { Some(b) => { mark(a, b); y } _ => { return 1; } } } _ => { return 0; } }",
        ),
        (
            "let r = make(); let Some(v) = r else { return y; }; v",
            "let r = make(); match r { Some(v) => { v } _ => { return y; } }",
        ),
        (
            "let Some(v) = o else { panic!() }; v",
            "match o { Some(v) => { v } _ => { panic!() } }",
        ),
    ] {
        equivalent(&optional(written), &optional(matched));
    }
    equivalent(
        "fn f(o: Option<u32>) { let Some(v) = o else { return; }; mark(v); }",
        "fn f(o: Option<u32>) { match o { Some(v) => { mark(v); } _ => { return; } } }",
    );
    equivalent(
        "fn f() { let _ = |o: Option<u32>| -> u32 { let Some(v) = o else { return 0; }; v }; }",
        "fn f() { let _ = |o: Option<u32>| -> u32 { match o { Some(v) => { v } _ => { return 0; } } }; }",
    );
    equivalent(
        "fn f(o: Option<u32>) { let Some(_) = o else { return; }; }",
        "fn f(o: Option<u32>) { match o { Some(_) => {} _ => { return; } } }",
    );
    equivalent(
        "fn f(o: Option<u32>) { let Some(v) = o else { return; }; println!(\"{v}\"); }",
        "fn f(o: Option<u32>) { match o { Some(v) => { println!(\"{v}\"); } _ => { return; } } }",
    );
    separate(
        "fn f(o: Option<u32>) -> u32 { let Some(v) = o else { return 0; }; mark(v); unreachable!() }",
        "fn f(o: Option<u32>) -> u32 { match o { Some(v) => { mark(v); unreachable!() } _ => { return 0; } } }",
    );
    equivalent(
        &function("match x { 0 => y, 1 | 2 => y, _ => x }"),
        &function("match x { 0 | 1 | 2 => y, _ => x }"),
    );
}

#[test]
fn let_else_with_temporaries_or_items_keeps_its_statement() {
    for (written, matched) in [
        (
            "let Some(v) = make() else { return y; }; v",
            "match make() { Some(v) => { v } _ => { return y; } }",
        ),
        (
            "let Some(v) = o else { return y; }; make()",
            "match o { Some(v) => { make() } _ => { return y; } }",
        ),
        (
            "let Some(v): Option<u32> = o else { return y; }; v",
            "match o { Some(v) => { v } _ => { return y; } }",
        ),
        (
            "const O: Option<u32> = None; let Some(v) = O else { return y; }; v",
            "const O: Option<u32> = None; match O { Some(v) => { v } _ => { return y; } }",
        ),
        (
            "let Some(v) = o else { return helper(); }; fn helper() -> u32 { 0 } v",
            "match o { Some(v) => { fn helper() -> u32 { 0 } v } _ => { return helper(); } }",
        ),
    ] {
        separate(&optional(written), &optional(matched));
    }
    separate(
        "fn f(o: Option<u32>) -> u32 { observe!({ let Some(v) = o else { return 0; }; v }) }",
        "fn f(o: Option<u32>) -> u32 { observe!({ match o { Some(v) => { v } _ => { return 0; } } }) }",
    );
    separate(
        "fn f(o: Option<u32>) { let Some(v) = o else { return; }; for _ in make() { mark(v); } }",
        "fn f(o: Option<u32>) { match o { Some(v) => { for _ in make() { mark(v); } } _ => { return; } } }",
    );
}

#[test]
fn function_views_join_arms_and_read_let_else_while_lexical_bodies_keep_them() {
    let file = parse(
        "fn first(x: u32, y: u32) -> u32 { match x { 0 => y, 1 => y, _ => x } } fn second(a: u32, b: u32) -> u32 { match a { 0 | 1 => b, _ => a } }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "fn first(x: u32, y: u32) -> u32 { match x { 0 => y, 1 => y, _ => x } } fn second(x: u32, y: u32) -> u32 { match x { 0 | 1 => y, _ => x } }",
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
    let file = parse(
        "fn first(o: Option<u32>) -> u32 { let Some(v) = o else { return 0; }; v } fn second(p: Option<u32>) -> u32 { match p { Some(w) => { w } _ => { return 0; } } }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
}
