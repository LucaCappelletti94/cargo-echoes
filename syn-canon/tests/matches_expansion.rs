//! A standard `matches!` reads as the `match` it expands to, and a negated boolean `match`
//! reads with its arm values negated.

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
fn matches_reads_as_its_match() {
    for (written, read) in [
        (
            "enum E { A, B, C }\nfn f(e: E) -> bool { matches!(e, E::A | E::B) }",
            "enum E { A, B, C }\nfn f(e: E) -> bool { match e { E::A | E::B => true, _ => false } }",
        ),
        (
            "fn f(o: Option<u32>) -> bool { matches!(o, Some(v) if v > 3) }",
            "fn f(o: Option<u32>) -> bool { match o { Some(v) if v > 3 => true, _ => false } }",
        ),
        (
            "fn g() -> Option<u32> { None }\nfn f() -> bool { matches!(g(), Some(_)) }",
            "fn g() -> Option<u32> { None }\nfn f() -> bool { match g() { Some(_) => true, _ => false } }",
        ),
        (
            "fn f(t: (u8, char)) -> bool { matches!(t, (0, 'a'..='z')) }",
            "fn f(t: (u8, char)) -> bool { match t { (0, 'a'..='z') => true, _ => false } }",
        ),
        (
            "fn f(o: Option<u32>) -> bool { matches!(o, Some(1..=5),) }",
            "fn f(o: Option<u32>) -> bool { match o { Some(1..=5) => true, _ => false } }",
        ),
        (
            "fn f(o: Option<u32>) -> bool { if let Some(_) = o { true } else { false } }",
            "fn f(o: Option<u32>) -> bool { matches!(o, Some(_)) }",
        ),
        (
            "fn f(x: u32) -> bool { match x { 1 | 2 => true, _ => false } }",
            "fn f(x: u32) -> bool { x == 1 || x == 2 }",
        ),
        (
            "enum E { A, B }\nfn second() -> u8 { 2 }\nfn first(e: E) -> bool { matches!(e, E::A) }\nfn main() { first(E::B); second(); }",
            "enum E { A, B }\nfn first(e: E) -> bool { match e { E::A => true, _ => false } }\nfn main() { first(E::B); second(); }\nfn second() -> u8 { 2 }",
        ),
    ] {
        equivalent(written, read);
    }
}

#[test]
fn negated_boolean_matches_negate_their_arms() {
    for (written, read) in [
        (
            "fn f(o: Option<u32>) -> bool { !matches!(o, Some(_)) }",
            "fn f(o: Option<u32>) -> bool { match o { Some(_) => false, _ => true } }",
        ),
        (
            "fn f(o: Option<u32>) -> bool { !match o { Some(_) => true, _ => false } }",
            "fn f(o: Option<u32>) -> bool { match o { Some(_) => false, _ => true } }",
        ),
        (
            "enum E { A, B, C }\nfn f(e: E) -> bool { !(match e { E::A => true, E::B => false, E::C => true }) }",
            "enum E { A, B, C }\nfn f(e: E) -> bool { match e { E::A => false, E::B => true, E::C => false } }",
        ),
    ] {
        equivalent(written, read);
    }
}

#[test]
fn local_macros_and_other_arm_values_stay_apart() {
    for (left, right) in [
        (
            "macro_rules! matches { ($e:expr, $p:pat) => { false }; }\nfn f(o: Option<u32>) -> bool { matches!(o, Some(_)) }",
            "macro_rules! matches { ($e:expr, $p:pat) => { false }; }\nfn f(o: Option<u32>) -> bool { match o { Some(_) => true, _ => false } }",
        ),
        (
            "fn f(o: Option<u32>) -> bool { matches!(o, Some(_)) }",
            "fn f(o: Option<u32>) -> bool { match o { Some(_) => false, _ => true } }",
        ),
        (
            "fn f(o: Option<u32>, p: bool) -> bool { !match o { Some(_) => p, _ => false } }",
            "fn f(o: Option<u32>, p: bool) -> bool { match o { Some(_) => !p, _ => true } }",
        ),
        (
            "fn f(o: Option<u32>) -> bool { matches!(o; Some(_)) }",
            "fn f(o: Option<u32>) -> bool { match o { Some(_) => true, _ => false } }",
        ),
    ] {
        separate(left, right);
    }
}
