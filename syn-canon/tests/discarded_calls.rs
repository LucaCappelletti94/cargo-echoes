//! `let _ = e;` over a call, method call or `.await` reads as the statement `e;`.

#[path = "support/canon.rs"]
mod canon;
#[path = "support/parse.rs"]
mod parse;
use canon::{equivalent, separate};

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
