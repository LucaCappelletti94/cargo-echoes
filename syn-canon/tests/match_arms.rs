//! `if let` reads as `match`, prelude variant arms join and disjoint arms take one order.

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

fn body(text: &str) -> String {
    format!("fn f(o: Option<u32>, r: Result<u32, u8>, x: u32, p: bool) -> u32 {{ {text} }}")
}

#[test]
fn if_let_reads_as_match() {
    for (written, matched) in [
        (
            "if let Some(v) = o { v } else { 0 }",
            "match o { None => 0, Some(v) => v }",
        ),
        (
            "if let Some(v) = o { g(v); } 0",
            "match o { Some(v) => { g(v); } _ => {} } 0",
        ),
        (
            "if let Some(v) = o { a(v) } else if p { b() } else { c() }",
            "match o { Some(v) => { a(v) } _ => if p { b() } else { c() } }",
        ),
        (
            "let n = o; if let Some(v) = n { v } else { x }",
            "let n = o; match n { Some(v) => v, None => x }",
        ),
    ] {
        equivalent(&body(written), &body(matched));
    }
    equivalent(
        "fn f() -> u32 { let k = |o: Option<u32>| if let Some(v) = o { v } else { 1 }; k(None) }",
        "fn f() -> u32 { let k = |o: Option<u32>| match o { Some(v) => v, None => 1 }; k(None) }",
    );
}

#[test]
fn prelude_variant_arms_read_as_catch_alls_and_join() {
    for (written, read) in [
        (
            "match o { Some(v) => a(v), None => b() }",
            "match o { Some(v) => a(v), _ => b() }",
        ),
        (
            "match o { None => b(), Some((v)) => a(v) }",
            "match o { Some(v) => a(v), _ => b() }",
        ),
        (
            "match o { None => 1, Some(0) => 1, Some(_) => 2 }",
            "match o { None | Some(0) => 1, Some(_) => 2 }",
        ),
        (
            "match r { Err(e) => u32::from(e), Ok(v) => v }",
            "match r { Ok(v) => v, Err(e) => u32::from(e) }",
        ),
    ] {
        equivalent(&body(written), &body(read));
    }
}

#[test]
fn disjoint_arms_take_one_order() {
    for (written, read) in [
        (
            "match x { 1 => a(), 0 => b(), _ => c() }",
            "match x { 0 => b(), 1 => a(), _ => c() }",
        ),
        (
            "match x { 2 => a(), 0 => b(), 1 => a(), _ => c() }",
            "match x { 0 => b(), 1 | 2 => a(), _ => c() }",
        ),
        (
            "match x { 0x10 => a(), 1_0 => b(), _ => c() }",
            "match x { 10 => b(), 16 => a(), _ => c() }",
        ),
        (
            "match p { true => a(), false => b() }",
            "match p { false => b(), true => a() }",
        ),
    ] {
        equivalent(&body(written), &body(read));
    }
    equivalent(
        "enum E { A, B(u32), C { v: u32 } } fn f(e: E) -> u32 { match e { E::C { v } => v, E::A => 0, E::B(v) => v + 1 } }",
        "enum E { A, B(u32), C { v: u32 } } fn f(e: E) -> u32 { match e { E::A => 0, E::B(v) => v + 1, E::C { v } => v } }",
    );
    equivalent(
        "fn f(c: char) -> u32 { match c { 'b' => 1, 'a' => 2, _ => 3 } }",
        "fn f(c: char) -> u32 { match c { 'a' => 2, 'b' => 1, _ => 3 } }",
    );
}

#[test]
fn unproven_shapes_keep_their_spelling() {
    for (written, read) in [
        (
            "if let Some(v) = h() { v } else { 0 }",
            "match h() { Some(v) => v, _ => 0 }",
        ),
        (
            "if let Some(v) = o && p { v } else { 0 }",
            "match o { Some(v) if p => v, _ => 0 }",
        ),
        (
            "match x { 1 => a(), 0x1 => b(), _ => c() }",
            "match x { 0x1 => b(), 1 => a(), _ => c() }",
        ),
        (
            "match x { 0..=5 => a(), 3 => b(), _ => c() }",
            "match x { 3 => b(), 0..=5 => a(), _ => c() }",
        ),
        (
            "match x { 1 if p => a(), 0 => b(), _ => c() }",
            "match x { 0 => b(), 1 if p => a(), _ => c() }",
        ),
        (
            "match x { _ => a(), 1 => b(), 0 => c() }",
            "match x { _ => a(), 0 => c(), 1 => b() }",
        ),
        (
            "match o { Some(0) => a(0), None => b() }",
            "match o { Some(0) => a(0), _ => b() }",
        ),
        (
            "match o { Some(1) => a(1), Some(0) => b(), _ => c() }",
            "match o { Some(0) => b(), Some(1) => a(1), _ => c() }",
        ),
        (
            "const None: Option<u32> = Option::None; match o { Some(v) => a(v), None => b() }",
            "const None: Option<u32> = Option::None; match o { Some(v) => a(v), _ => b() }",
        ),
    ] {
        separate(&body(written), &body(read));
    }
    separate(
        "enum E { X, Y } use E::X as None; fn f(o: Option<u32>) -> u32 { match o { Some(v) => v, None => 0 } }",
        "enum E { X, Y } use E::X as None; fn f(o: Option<u32>) -> u32 { match o { Some(v) => v, _ => 0 } }",
    );
    separate(
        "mod E { pub const A: u32 = 1; pub const B: u32 = 1; } fn f(x: u32) -> u32 { match x { E::B => 1, E::A => 2, _ => 3 } }",
        "mod E { pub const A: u32 = 1; pub const B: u32 = 1; } fn f(x: u32) -> u32 { match x { E::A => 2, E::B => 1, _ => 3 } }",
    );
    separate(
        "#![no_implicit_prelude] fn f(o: ::std::option::Option<u32>) -> u32 { match o { Some(v) => v, None => 0 } }",
        "#![no_implicit_prelude] fn f(o: ::std::option::Option<u32>) -> u32 { match o { Some(v) => v, _ => 0 } }",
    );
    separate(
        "static S: Option<u32> = None; fn f() -> u32 { if let Some(v) = S { v } else { 0 } }",
        "static S: Option<u32> = None; fn f() -> u32 { match S { Some(v) => v, _ => 0 } }",
    );
}

#[test]
fn scoped_bindings_count_as_places() {
    for (written, read) in [
        (
            "for n in [o] { if let Some(v) = n { g(v); } } 0",
            "for n in [o] { match n { Some(v) => { g(v); } _ => {} } } 0",
        ),
        (
            "let mut it = [o].into_iter(); while let Some(n) = it.next() { if let Some(v) = n { g(v); } } 0",
            "let mut it = [o].into_iter(); while let Some(n) = it.next() { match n { Some(v) => { g(v); } _ => {} } } 0",
        ),
        (
            "if let Some(n) = h() && p { if let Some(v) = n { g(v); } } 0",
            "if let Some(n) = h() && p { match n { Some(v) => { g(v); } _ => {} } } 0",
        ),
        (
            "match x { k if k > 0 => if let Some(v) = o { v } else { k }, _ => 0 }",
            "match x { k if k > 0 => match o { Some(v) => v, None => k }, _ => 0 }",
        ),
        (
            "let Some(n) = o else { let q = x; return if let 0 = q { 1 } else { 2 }; }; n",
            "let Some(n) = o else { let q = x; return match q { 0 => 1, _ => 2 }; }; n",
        ),
        (
            "#[allow(unused_mut)] let mut n = o; if let Some(v) = n { v } else { 0 }",
            "#[allow(unused_mut)] let mut n = o; match n { Some(v) => v, None => 0 }",
        ),
        (
            "fn inner(o: Option<u32>) -> u32 { if let Some(v) = o { v } else { 0 } } inner(o)",
            "fn inner(o: Option<u32>) -> u32 { match o { Some(v) => v, None => 0 } } inner(o)",
        ),
        (
            "enum E { A, B } let e = E::B; match e { E::B => 1, E::A => 0 }",
            "enum E { A, B } let e = E::B; match e { E::A => 0, E::B => 1 }",
        ),
        (
            "struct S; impl S {} if let Some(v) = o { v } else { 0 }",
            "struct S; impl S {} match o { Some(v) => v, None => 0 }",
        ),
    ] {
        equivalent(&body(written), &body(read));
    }
}

#[test]
fn irrefutable_payloads_and_literal_kinds_take_one_spelling() {
    for (written, read) in [
        (
            "fn f(o: Option<(u32, &u32)>) -> u32 { match o { None => 0, Some((a, &b)) => a + b } }",
            "fn f(o: Option<(u32, &u32)>) -> u32 { match o { Some((a, &b)) => a + b, _ => 0 } }",
        ),
        (
            "fn f(o: Option<u32>) -> u32 { match o { None => 0, Some(..) => 1 } }",
            "fn f(o: Option<u32>) -> u32 { match o { Some(..) => 1, _ => 0 } }",
        ),
        (
            "fn f(o: Option<(u32, u32)>) -> u32 { match o { None => 0, Some(t @ (_, _)) => t.0 } }",
            "fn f(o: Option<(u32, u32)>) -> u32 { match o { Some(t @ (_, _)) => t.0, _ => 0 } }",
        ),
        (
            "fn f(x: u32) -> u32 { match x { 3 | 1 => a(), 0 => b(), _ => c() } }",
            "fn f(x: u32) -> u32 { match x { 0 => b(), 3 | 1 => a(), _ => c() } }",
        ),
        (
            "fn f(x: u8) -> u32 { match x { b'b' => 1, b'a' => 2, _ => 3 } }",
            "fn f(x: u8) -> u32 { match x { b'a' => 2, b'b' => 1, _ => 3 } }",
        ),
        (
            "fn f(x: &str) -> u32 { match x { \"b\" => 1, \"a\" => 2, _ => 3 } }",
            "fn f(x: &str) -> u32 { match x { \"a\" => 2, \"b\" => 1, _ => 3 } }",
        ),
        (
            "fn f(x: &[u8]) -> u32 { match x { b\"b\" => 1, b\"a\" => 2, _ => 3 } }",
            "fn f(x: &[u8]) -> u32 { match x { b\"a\" => 2, b\"b\" => 1, _ => 3 } }",
        ),
    ] {
        equivalent(written, read);
    }
    separate(
        "fn f(o: Option<u32>) -> u32 { match o { None => 0, Some(v @ 0..) => v } }",
        "fn f(o: Option<u32>) -> u32 { match o { Some(v @ 0..) => v, _ => 0 } }",
    );
}

#[test]
fn claimed_or_unresolved_names_keep_their_arms() {
    for (written, read) in [
        (
            "#[allow(unused_variables)] if let Some(v) = o { g(v); } 0",
            "#[allow(unused_variables)] match o { Some(v) => { g(v); } _ => {} } 0",
        ),
        (
            "match x { #[cfg(all())] 1 => a(), 0 => b(), _ => c() }",
            "match x { 0 => b(), #[cfg(all())] 1 => a(), _ => c() }",
        ),
        (
            "{ use m::*; if let Some(v) = o { v } else { 0 } }",
            "{ use m::*; match o { Some(v) => v, _ => 0 } }",
        ),
        (
            "use std::option::Option::*; match o { Some(v) => v, None => 0 }",
            "use std::option::Option::*; match o { Some(v) => v, _ => 0 }",
        ),
        (
            "struct None; match o { Some(v) => v, None => 0 }",
            "struct None; match o { Some(v) => v, _ => 0 }",
        ),
        (
            "fn Some(v: u32) -> u32 { v } match o { Some(v) => v, None => 0 }",
            "fn Some(v: u32) -> u32 { v } match o { Some(v) => v, _ => 0 }",
        ),
        (
            "use m::None::{self}; match o { Some(v) => v, None => 0 }",
            "use m::None::{self}; match o { Some(v) => v, _ => 0 }",
        ),
        (
            "type None = u8; match o { Some(v) => v, None => 0 }",
            "type None = u8; match o { Some(v) => v, _ => 0 }",
        ),
    ] {
        separate(&body(written), &body(read));
    }
    for (written, read) in [
        (
            "enum E { A, B } enum F { A, B } fn f(e: F) -> u32 { use F as E; match e { E::B => 1, E::A => 0 } }",
            "enum E { A, B } enum F { A, B } fn f(e: F) -> u32 { use F as E; match e { E::A => 0, E::B => 1 } }",
        ),
        (
            "enum E { A, B } fn f(e: E) -> u32 { { use m::*; match e { E::B => 1, E::A => 0 } } }",
            "enum E { A, B } fn f(e: E) -> u32 { { use m::*; match e { E::A => 0, E::B => 1 } } }",
        ),
        (
            "enum E { A, B } fn f(e: E) -> u32 { extern crate core as E; match e { E::B => 1, E::A => 0 } }",
            "enum E { A, B } fn f(e: E) -> u32 { extern crate core as E; match e { E::A => 0, E::B => 1 } }",
        ),
        (
            "enum G<T> { A(T), B(T) } fn f(g: G<u8>) -> u8 { match g { G::<u8>::B(v) => v, G::<u8>::A(v) => v + 1 } }",
            "enum G<T> { A(T), B(T) } fn f(g: G<u8>) -> u8 { match g { G::<u8>::A(v) => v + 1, G::<u8>::B(v) => v } }",
        ),
        (
            "mod m { pub enum E { A, B } } fn f(e: m::E) -> u32 { match e { m::E::B => 1, m::E::A => 0 } }",
            "mod m { pub enum E { A, B } } fn f(e: m::E) -> u32 { match e { m::E::A => 0, m::E::B => 1 } }",
        ),
        (
            "enum E { A, B } use E::{A, B}; fn f(e: E) -> u32 { match e { B => 1, A => 0 } }",
            "enum E { A, B } use E::{A, B}; fn f(e: E) -> u32 { match e { A => 0, B => 1 } }",
        ),
        (
            "fn f(y: f64) -> u32 { match y { 1.5 => 1, 0.5 => 2, _ => 3 } }",
            "fn f(y: f64) -> u32 { match y { 0.5 => 2, 1.5 => 1, _ => 3 } }",
        ),
        (
            "use std::option::Option::*; fn f(o: Option<u32>) -> u32 { match o { Some(v) => v, None => 0 } }",
            "use std::option::Option::*; fn f(o: Option<u32>) -> u32 { match o { Some(v) => v, _ => 0 } }",
        ),
    ] {
        separate(written, read);
    }
}
