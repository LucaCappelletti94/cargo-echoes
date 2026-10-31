//! A `loop` whose only content is a `match` that breaks on every other value reads as the
//! `while let` it is the expansion of.

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, read: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(read));
    assert_eq!(left, right, "{written}\n{read}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(read)),
        "{written}\n{read}"
    );
}

fn separate(left: &str, right: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(left)),
            canonicalize(parse(right)),
            "{left}\n{right}"
        );
    }
}

#[test]
fn breaking_matches_read_as_while_let() {
    for (written, read) in [
        (
            "fn f() { loop { match next() { Some(x) => g(x), None => break } } }",
            "fn f() { while let Some(x) = next() { g(x) } }",
        ),
        (
            "fn f(it: &mut I) { loop { match it.next() { Some(x) => { g(x); } _ => break } } }",
            "fn f(it: &mut I) { while let Some(x) = it.next() { g(x); } }",
        ),
        (
            "fn f(it: &mut I) { loop { match it.next() { None => break, Some(x) => g(x) } } }",
            "fn f(it: &mut I) { while let Some(x) = it.next() { g(x) } }",
        ),
        (
            "fn f(mut o: Option<u32>) { loop { match o { Some(x) => { o = step(x); } None => break } } }",
            "fn f(mut o: Option<u32>) { while let Some(x) = o { o = step(x); } }",
        ),
        (
            "fn f() { loop { match read() { Ok(v) => g(v), _ => break } } }",
            "fn f() { while let Ok(v) = read() { g(v) } }",
        ),
        (
            "fn f() { loop { match next() { Some(x) => { if x == 0 { continue; } if x > 9 { break; } g(x); } None => break } } }",
            "fn f() { while let Some(x) = next() { if x == 0 { continue; } if x > 9 { break; } g(x); } }",
        ),
        (
            "fn f() { 'outer: for y in ys() { loop { match next() { Some(x) => { if x == y { break 'outer; } } None => break } } } }",
            "fn f() { 'outer: for y in ys() { while let Some(x) = next() { if x == y { break 'outer; } } } }",
        ),
        (
            "fn f() { loop { match next() { Some(x) => g(x), _ => { break } } } }",
            "fn f() { while let Some(x) = next() { g(x) } }",
        ),
        (
            "fn f() { loop { match next() { Some(x) => { let v = loop { break x; }; while ready() { break; } for y in ys() { g(y); } let c = || x; let a = async { x }; fn helper() {} g(v); } None => break } } }",
            "fn f() { while let Some(x) = next() { let v = loop { break x; }; while ready() { break; } for y in ys() { g(y); } let c = || x; let a = async { x }; fn helper() {} g(v); } }",
        ),
    ] {
        equivalent(written, read);
    }
}

#[test]
fn valued_labelled_guarded_and_let_else_loops_stay_apart() {
    for (left, right) in [
        (
            "fn f() { loop { match next() { Some(x) => { if x > 1 { break 1; } } None => break } } }",
            "fn f() { while let Some(x) = next() { if x > 1 { break 1; } } }",
        ),
        (
            "fn f() { 'outer: loop { match next() { Some(x) => g(x), None => break } } }",
            "fn f() { 'outer: while let Some(x) = next() { g(x) } }",
        ),
        (
            "fn f() { loop { match next() { Some(x) if x > 1 => g(x), _ => break } } }",
            "fn f() { while let Some(x) = next() { g(x) } }",
        ),
        (
            "fn f() { loop { let Some(x) = next() else { break }; g(x); } }",
            "fn f() { while let Some(x) = next() { g(x); } }",
        ),
        (
            "fn f() { loop { h(); match next() { Some(x) => g(x), None => break } } }",
            "fn f() { h(); while let Some(x) = next() { g(x) } }",
        ),
        (
            "fn f() { loop { match next() { Some(x) => g(x), None => return } } }",
            "fn f() { while let Some(x) = next() { g(x) } }",
        ),
        (
            "fn f() { loop { match next() { Some(x) => g(x), _ => { h(); break } } } }",
            "fn f() { while let Some(x) = next() { g(x) } }",
        ),
        (
            "fn f() { loop { match next() { Some(0) => h(), Some(x) => g(x), None => break } } }",
            "fn f() { while let Some(x) = next() { g(x) } }",
        ),
        (
            "fn f() { loop { match next() { Some(x) => g(x), None => break } }; }",
            "fn f() { loop { match next() { Some(x) => g(x), None => continue } }; }",
        ),
    ] {
        separate(left, right);
    }
}
