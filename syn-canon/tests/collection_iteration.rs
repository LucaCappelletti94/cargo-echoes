//! Iteration over proven standard collections and their emptiness tests take one spelling.

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

fn separate(written: &str, read: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(read)),
            "{written}\n{read}"
        );
    }
}

#[test]
fn iteration_reads_as_borrowed_loops() {
    for (written, read) in [
        (
            "fn f(v: Vec<u32>) { for x in v.iter() { g(x); } }",
            "fn f(v: Vec<u32>) { for x in &v { g(x); } }",
        ),
        (
            "fn f(v: &Vec<u32>) { for x in v.iter() { g(x); } }",
            "fn f(v: &Vec<u32>) { for x in v { g(x); } }",
        ),
        (
            "fn f(v: &[u32]) { for x in v.iter() { g(x); } }",
            "fn f(v: &[u32]) { for x in v { g(x); } }",
        ),
        (
            "fn f(v: [u32; 3]) { for x in v.iter() { g(x); } }",
            "fn f(v: [u32; 3]) { for x in &v { g(x); } }",
        ),
        (
            "fn f(mut v: Vec<u32>) { for x in v.iter_mut() { *x += 1; } }",
            "fn f(mut v: Vec<u32>) { for x in &mut v { *x += 1; } }",
        ),
        (
            "fn f(v: &mut [u32]) { for x in v.iter_mut() { *x += 1; } }",
            "fn f(v: &mut [u32]) { for x in v { *x += 1; } }",
        ),
        (
            "fn f(v: Vec<u32>) { for x in v.into_iter() { g(x); } }",
            "fn f(v: Vec<u32>) { for x in v { g(x); } }",
        ),
        (
            "fn f() { let v: Vec<u32> = Vec::new(); for x in v.iter() { g(x); } }",
            "fn f() { let v: Vec<u32> = Vec::new(); for x in &v { g(x); } }",
        ),
    ] {
        equivalent(written, read);
    }
}

#[test]
fn length_tests_read_as_emptiness() {
    for (written, read) in [
        ("v.len() == 0", "v.is_empty()"),
        ("0 == v.len()", "v.is_empty()"),
        ("v.len() < 1", "v.is_empty()"),
        ("v.len() != 0", "!v.is_empty()"),
        ("v.len() > 0", "!v.is_empty()"),
        ("0 < v.len()", "!v.is_empty()"),
        ("v.len() >= 1", "!v.is_empty()"),
        ("s.len() == 0usize", "s.is_empty()"),
        ("t.len() == 0", "t.is_empty()"),
        ("a.len() != 0", "!a.is_empty()"),
    ] {
        equivalent(
            &format!(
                "fn f(v: &[u32], s: String, t: &str, a: &mut Vec<u8>) -> bool {{ {written} }}"
            ),
            &format!("fn f(v: &[u32], s: String, t: &str, a: &mut Vec<u8>) -> bool {{ {read} }}"),
        );
    }
}

#[test]
fn unproven_collections_keep_their_spelling() {
    for (written, read) in [
        (
            "fn f(v: T) { for x in v.iter() { g(x); } }",
            "fn f(v: T) { for x in &v { g(x); } }",
        ),
        (
            "fn f() { let v = make(); for x in v.iter() { g(x); } }",
            "fn f() { let v = make(); for x in &v { g(x); } }",
        ),
        (
            "fn f(v: [u32; 3]) { for x in v.into_iter() { g(x); } }",
            "fn f(v: [u32; 3]) { for x in v { g(x); } }",
        ),
        (
            "fn f(v: &mut Vec<u32>) { for x in v.iter() { g(x); } }",
            "fn f(v: &mut Vec<u32>) { for x in v { g(x); } }",
        ),
        (
            "struct Vec<T>(T); fn f(v: Vec<u32>) -> bool { v.len() == 0 }",
            "struct Vec<T>(T); fn f(v: Vec<u32>) -> bool { v.is_empty() }",
        ),
        (
            "trait Size { fn len(&self) -> usize; } fn f(v: Vec<u32>) -> bool { v.len() == 0 }",
            "trait Size { fn len(&self) -> usize; } fn f(v: Vec<u32>) -> bool { v.is_empty() }",
        ),
        (
            "use other::Thing; fn f(v: Vec<u32>) -> bool { v.len() == 0 }",
            "use other::Thing; fn f(v: Vec<u32>) -> bool { v.is_empty() }",
        ),
        (
            "fn f(v: Vec<u32>) -> bool { v.len() == 1 }",
            "fn f(v: Vec<u32>) -> bool { v.is_empty() }",
        ),
        (
            "fn f(v: Vec<u32>) -> bool { let v = 3u32; v.len() == 0 }",
            "fn f(v: Vec<u32>) -> bool { let v = 3u32; v.is_empty() }",
        ),
        (
            "#![no_implicit_prelude] fn f(v: &[u32]) -> bool { v.len() == 0 }",
            "#![no_implicit_prelude] fn f(v: &[u32]) -> bool { v.is_empty() }",
        ),
    ] {
        separate(written, read);
    }
}

#[test]
fn scoped_bindings_and_other_loops_keep_the_readings_consistent() {
    for (written, read) in [
        (
            "fn f(o: Option<u32>, v: &[u32]) -> bool { if let Some(x) = o { g(x); } let w: Vec<u32>; while v.len() > 0 { g(1); } for i in 0..3 { g(i); } for c in v.chunks(2) { g(c); } match o { Some(1..=5) => true, Some(n) if n > v.len() as u32 => false, _ => v.len() == 0 } }",
            "fn f(o: Option<u32>, v: &[u32]) -> bool { if let Some(x) = o { g(x); } let w: Vec<u32>; while !v.is_empty() { g(1); } for i in 0..3 { g(i); } for c in v.chunks(2) { g(c); } match o { Some(1..=5) => true, Some(n) if n > v.len() as u32 => false, _ => v.is_empty() } }",
        ),
        (
            "fn f() -> bool { let k = |v: &[u32]| v.len() == 0; k(&[]) }",
            "fn f() -> bool { let k = |v: &[u32]| v.is_empty(); k(&[]) }",
        ),
    ] {
        equivalent(written, read);
    }
    for (written, read) in [
        (
            "fn f(v: Vec<u32>) -> bool { use std::collections::*; v.len() == 0 }",
            "fn f(v: Vec<u32>) -> bool { use std::collections::*; v.is_empty() }",
        ),
        (
            "fn f(v: Vec<u32>) -> bool { fn helper<Vec>(x: Vec) -> Vec { x } v.len() == 0 }",
            "fn f(v: Vec<u32>) -> bool { fn helper<Vec>(x: Vec) -> Vec { x } v.is_empty() }",
        ),
        (
            "fn f(v: std::vec::Vec<u32>) -> bool { v.len() == 0 }",
            "fn f(v: std::vec::Vec<u32>) -> bool { v.is_empty() }",
        ),
        (
            "struct S { v: Vec<u32> } fn f(s: S) -> bool { s.v.len() == 0 }",
            "struct S { v: Vec<u32> } fn f(s: S) -> bool { s.v.is_empty() }",
        ),
        (
            "fn f(s: &str) { for c in s.iter() { g(c); } }",
            "fn f(s: &str) { for c in s { g(c); } }",
        ),
        (
            "fn f(v: &[u32], n: usize) -> bool { v.len() == n }",
            "fn f(v: &[u32], n: usize) -> bool { v.is_empty() }",
        ),
        (
            "fn f(v: &[u32]) -> bool { v.len() == 0u32 }",
            "fn f(v: &[u32]) -> bool { v.is_empty() }",
        ),
        (
            "fn f(v: &[u32]) -> bool { 1 == v.len() }",
            "fn f(v: &[u32]) -> bool { v.is_empty() }",
        ),
        (
            "fn f(v: &[u32]) -> bool { v.len() == 0 && #[cfg(all())] { v.len() == 0 } }",
            "fn f(v: &[u32]) -> bool { v.is_empty() && #[cfg(all())] { v.is_empty() } }",
        ),
    ] {
        separate(written, read);
    }
}
