//! `matches!`, literal disjunctions and literal ranges over a proven integer place.

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

fn test(ty: &str, body: &str) -> String {
    format!("fn f(x: {ty}) -> bool {{ {body} }}")
}

#[test]
fn chains_ranges_matches_and_contains_read_as_one_form() {
    for (ty, written, read) in [
        ("u32", "matches!(x, 1 | 2)", "x == 1 || x == 2"),
        ("u32", "matches!(x, 2 | 1)", "x == 1 || x == 2"),
        ("u32", "x == 2 || x == 1", "matches!(x, 1 | 2)"),
        ("u8", "matches!(x, 7)", "x == 7"),
        ("u32", "matches!(x, 1..=5)", "x >= 1 && x <= 5"),
        ("i64", "matches!(x, 3..7)", "x >= 3 && x < 7"),
        ("u16", "matches!(x, 10..)", "x >= 10"),
        ("i8", "(..=4).contains(&x)", "x <= 4"),
        (
            "u32",
            "matches!(x, 1 | 3..=5)",
            "x == 1 || x >= 3 && x <= 5",
        ),
        ("i32", "matches!(x, -1 | 1)", "x == -1 || x == 1"),
        ("u32", "matches!(x, (1 | 2))", "x == 1 || x == 2"),
        ("u128", "matches!(x, 1 | 2)", "x == 1 || x == 2"),
        (
            "u32",
            "matches!(x, 1 | 2) && x != 5",
            "(x == 1 || x == 2) && x != 5",
        ),
        ("u32", "matches!(x, | 4 | 8,)", "x == 4 || x == 8"),
        ("u32", "std::matches!(x, 1 | 2)", "x == 1 || x == 2"),
        ("u32", "(1..=5).contains(&x)", "x >= 1 && x <= 5"),
        ("u32", "(1..5).contains(&x)", "matches!(x, 1..5)"),
        ("u64", "matches!(x, 1u64 | 2)", "x == 1u64 || x == 2"),
    ] {
        equivalent(&test(ty, written), &test(ty, read));
    }
}

#[test]
fn guards_comparands_scrutinees_overlaps_and_shadowing_stay_apart() {
    for (written, read) in [
        (
            test("u32", "matches!(x, 1 | 2 if x > 0)"),
            test("u32", "x == 1 || x == 2"),
        ),
        (
            "const Y: u32 = 3; fn f(x: u32) -> bool { matches!(x, Y) }".to_owned(),
            "const Y: u32 = 3; fn f(x: u32) -> bool { x == Y }".to_owned(),
        ),
        (
            "fn g() -> u32 { 1 } fn f() -> bool { matches!(g(), 1 | 2) }".to_owned(),
            "fn g() -> u32 { 1 } fn f() -> bool { g() == 1 || g() == 2 }".to_owned(),
        ),
        (
            test("u32", "matches!(x, 1 | 1..=3)"),
            test("u32", "x == 1 || x >= 1 && x <= 3"),
        ),
        (
            test("u32", "matches!(x, 1u64 | 2)"),
            test("u32", "x == 1u64 || x == 2"),
        ),
        (
            test("u32", "matches!(x, 5..=1)"),
            test("u32", "x >= 5 && x <= 1"),
        ),
        (
            "macro_rules! matches { ($($t:tt)*) => { true } } fn f(x: u32) -> bool { matches!(x, 1 | 2) }"
                .to_owned(),
            "macro_rules! matches { ($($t:tt)*) => { true } } fn f(x: u32) -> bool { x == 1 || x == 2 }"
                .to_owned(),
        ),
        (
            "trait T { fn contains(&self, x: &u32) -> bool; } fn f(x: u32) -> bool { (1..=5).contains(&x) }"
                .to_owned(),
            "trait T { fn contains(&self, x: &u32) -> bool; } fn f(x: u32) -> bool { x >= 1 && x <= 5 }"
                .to_owned(),
        ),
        (
            test("u32", "(1..=5).contains(x)"),
            test("u32", "x >= 1 && x <= 5"),
        ),
        (
            test("u32", "(1..=5).contains(&mut x)"),
            test("u32", "x >= 1 && x <= 5"),
        ),
        (
            "fn f(x: u32, v: &[u32]) -> bool { v.contains(&x) }".to_owned(),
            "fn f(x: u32, v: &[u32]) -> bool { x == 1 }".to_owned(),
        ),
        (
            "const Y: u32 = 5; fn f(x: u32) -> bool { matches!(x, 1..=Y) }".to_owned(),
            "const Y: u32 = 5; fn f(x: u32) -> bool { x >= 1 && x <= Y }".to_owned(),
        ),
        (test("bool", "matches!(x, 1)"), test("bool", "x == 1")),
        (test("u32", "(..).contains(&x)"), test("u32", "true")),
    ] {
        separate(&written, &read);
    }
}
