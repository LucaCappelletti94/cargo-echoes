//! `let _ = e;` over a call, method call or `.await` reads as the statement `e;`.

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, dropped: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(dropped));
    assert_eq!(left, right, "{written}\n{dropped}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(dropped)),
        "{written}\n{dropped}"
    );
}

fn separate(written: &str, dropped: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(dropped)),
            "{written}\n{dropped}"
        );
    }
}

fn body(text: &str) -> String {
    format!("fn f(x: Vec<u32>, s: S) {{ {text} }}")
}

#[test]
fn discarded_call_results_read_as_statements() {
    for (bound, dropped) in [
        ("let _ = g();", "g();"),
        ("let _ = s.get(1); h();", "s.get(1); h();"),
        ("let _ = g(x.len()); let _ = h();", "g(x.len()); h();"),
        ("{ let _ = g(); } h();", "{ g(); } h();"),
        ("#[allow(unused_must_use)] let _ = g(); h();", "g(); h();"),
        (
            "let k = || { let _ = g(); }; k();",
            "let k = || { g(); }; k();",
        ),
    ] {
        equivalent(&body(bound), &body(dropped));
    }
    equivalent(
        "async fn f(t: T) { let _ = t.await; }",
        "async fn f(t: T) { t.await; }",
    );
}

#[test]
fn other_bindings_keep_their_let() {
    for (bound, dropped) in [
        ("let _ = x; h();", "x; h();"),
        ("let _: Vec<u32> = g(); h();", "g(); h();"),
        ("let _y = g(); h();", "g(); h();"),
        ("#[cfg(all())] let _ = g(); h();", "#[cfg(all())] g(); h();"),
        ("let _ = g()?; h();", "g()?; h();"),
        ("let _ = vec![g()]; h();", "vec![g()]; h();"),
        ("let _ = s.0; h();", "s.0; h();"),
    ] {
        separate(&body(bound), &body(dropped));
    }
}
