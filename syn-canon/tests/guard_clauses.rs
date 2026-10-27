//! Guard clauses in unit and loop bodies read as conditionals.

#[path = "support/view.rs"]
mod view;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, canonical: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(canonical));
    assert_eq!(left, right, "{written}\n{canonical}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(canonical)),
        "{written}\n{canonical}"
    );
}

fn separate(written: &str, canonical: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(canonical)),
            "{written}\n{canonical}"
        );
    }
}

fn unit(body: &str) -> String {
    format!("fn f(p: bool, q: bool, x: u32) {{ {body} }}")
}

#[test]
fn return_guards_read_as_conditionals() {
    for (guarded, branched) in [
        (
            "if p { a(); return; } b(); c();",
            "if p { a(); } else { b(); c(); }",
        ),
        ("if p { return; } b();", "if !p { b(); }"),
        ("a(); if p { return } b();", "a(); if !p { b(); }"),
        (
            "if p { return; } if q { a(); return; } b();",
            "if !p { if q { a(); } else { b(); } }",
        ),
        (
            "if p { if q { return; } a(); return; } b();",
            "if p { if !q { a(); } } else { b(); }",
        ),
        ("if !p { return; } b();", "if p { b(); }"),
        (
            "if p { return; } let y = x; b(y);",
            "if !p { let y = x; b(y); }",
        ),
        ("if p { return; } b(); ", "if !p { b(); }"),
    ] {
        equivalent(&unit(guarded), &unit(branched));
    }
    equivalent(
        "fn f() { let _ = |p: bool| { if p { return; } b(); }; }",
        "fn f() { let _ = |p: bool| { if !p { b(); } }; }",
    );
    equivalent(
        "impl S { fn f(p: bool) { if p { return; } b(); } }",
        "impl S { fn f(p: bool) { if !p { b(); } } }",
    );
}

#[test]
fn loop_guards_read_as_conditionals() {
    for (guarded, branched) in [
        (
            "for v in [x] { if x == 0 { continue; } g(v); }",
            "for v in [x] { if x != 0 { g(v); } }",
        ),
        (
            "for v in [x] { a(); if p { b(); continue; } g(v); }",
            "for v in [x] { a(); if p { b(); } else { g(v); } }",
        ),
        (
            "'outer: for v in [x] { if p { continue 'outer; } g(v); }",
            "'outer: for v in [x] { if !p { g(v); } }",
        ),
        (
            "while q { if p { continue; } g(x); }",
            "while q { if !p { g(x); } }",
        ),
        (
            "let mut i = x; loop { if p { break; } i -= 1; g(i); }",
            "let mut i = x; while !p { i -= 1; g(i); }",
        ),
        (
            "let mut i = x; 'scan: loop { if p { break 'scan; } i -= 1; g(i); }",
            "let mut i = x; 'scan: while !p { i -= 1; g(i); }",
        ),
    ] {
        equivalent(&unit(guarded), &unit(branched));
    }
}

#[test]
fn other_guards_keep_their_jump() {
    for (guarded, branched) in [
        (
            "if p { return; } fn b() {} b();",
            "if !p { fn b() {} b(); }",
        ),
        (
            "if p { return; } for v in [x] { g(v); }",
            "if !p { for v in [x] { g(v); } }",
        ),
        (
            "if let Some(v) = Some(x) { g(v); return; } b();",
            "if let Some(v) = Some(x) { g(v); } else { b(); }",
        ),
        ("if p { return; } else { a(); } b();", "if !p { a(); b(); }"),
        ("{ if p { return; } a(); } b();", "{ if !p { a(); } } b();"),
        (
            "'outer: for v in [x] { for w in [v] { if p { continue 'outer; } g(w); } }",
            "'outer: for v in [x] { for w in [v] { if !p { g(w); } } }",
        ),
        (
            "let mut i = x; loop { if i == 0 { break; } i -= 1; } g(i);",
            "let mut i = x; while i > 0 { i -= 1; } g(i);",
        ),
        (
            "let _ = loop { if p { break 1u32; } a(); };",
            "let _ = while !p { a(); };",
        ),
        (
            "loop { if p { break; } macro_rules! m { () => {} } m!(); }",
            "while !p { macro_rules! m { () => {} } m!(); }",
        ),
        (
            "loop { a(); if p { break; } b(); }",
            "loop { a(); while !p { b(); } }",
        ),
    ] {
        separate(&unit(guarded), &unit(branched));
    }
    separate(
        "async fn f(p: bool) { if p { return; } b(); }",
        "async fn f(p: bool) { if !p { b(); } }",
    );
    separate(
        "impl S { async fn f(p: bool) { if p { return; } b(); } }",
        "impl S { async fn f(p: bool) { if !p { b(); } } }",
    );
    separate(
        "#[observe] fn f(p: bool) { if p { return; } b(); }",
        "#[observe] fn f(p: bool) { if !p { b(); } }",
    );
}

#[test]
fn function_views_read_guard_clauses() {
    let file =
        parse("fn first(p: bool) { if p { return; } b(); } fn second(q: bool) { if !q { b(); } }");
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
}
