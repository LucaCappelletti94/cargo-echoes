//! Bound order outside `where` clauses and impl lifetimes the header names once.

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

#[test]
fn bounds_outside_where_clauses_match_in_any_order() {
    for (written, sorted) in [
        (
            "fn f(x: Box<dyn Sync + Send>) {}",
            "fn f(x: Box<dyn Send + Sync>) {}",
        ),
        (
            "fn f<'a>(x: Box<dyn Send + 'a + Sync>) {}",
            "fn f<'a>(x: Box<dyn 'a + Send + Sync>) {}",
        ),
        (
            "fn f(x: impl Debug + Clone) {}",
            "fn f(x: impl Clone + Debug) {}",
        ),
        (
            "fn f() -> impl Sync + Send { 1 }",
            "fn f() -> impl Send + Sync { 1 }",
        ),
        ("trait T: Sync + Send {}", "trait T: Send + Sync {}"),
        (
            "trait T { type A: Debug + Clone; }",
            "trait T { type A: Clone + Debug; }",
        ),
        (
            "fn f(x: impl Iterator<Item: Debug + Clone>) {}",
            "fn f(x: impl Iterator<Item: Clone + Debug>) {}",
        ),
        (
            "fn f<T, U>(x: Box<dyn AsRef<U> + AsRef<T>>) {}",
            "fn f<T, U>(x: Box<dyn AsRef<T> + AsRef<U>>) {}",
        ),
        (
            "fn f<B, A>(x: Box<dyn AsRef<A> + AsRef<B>>) {}",
            "fn f<T, U>(x: Box<dyn AsRef<T> + AsRef<U>>) {}",
        ),
        (
            "struct S<T>(T); impl<T> S<T> { fn f<U>(x: Box<dyn AsRef<U> + AsRef<T>>) {} }",
            "struct S<A>(A); impl<A> S<A> { fn f<B>(x: Box<dyn AsRef<A> + AsRef<B>>) {} }",
        ),
        (
            "extern \"C\" { fn g(x: *const u8); } fn f<const N: usize>(x: Box<dyn Sync + AsRef<[u8; N]>>) {}",
            "extern \"C\" { fn g(x: *const u8); } fn f<const M: usize>(x: Box<dyn AsRef<[u8; M]> + Sync>) {}",
        ),
    ] {
        equivalent(written, sorted);
    }
}

#[test]
fn fn_family_bindings_and_capture_lists_keep_their_order() {
    for (written, sorted) in [
        (
            "fn f(x: Box<dyn Send + Fn()>) {}",
            "fn f(x: Box<dyn Fn() + Send>) {}",
        ),
        (
            "fn f(x: Box<dyn Send + Iterator<Item = u8>>) {}",
            "fn f(x: Box<dyn Iterator<Item = u8> + Send>) {}",
        ),
        (
            "fn f() -> impl Sync + Send + use<> { 1 }",
            "fn f() -> impl Send + Sync + use<> { 1 }",
        ),
    ] {
        separate(written, sorted);
    }
}

#[test]
fn impl_lifetimes_the_header_names_once_read_as_anonymous() {
    for (named, anonymous) in [
        (
            "struct S<'a>(&'a u8); impl<'a> S<'a> { fn get(&self) -> u8 { *self.0 } }",
            "struct S<'a>(&'a u8); impl S<'_> { fn get(&self) -> u8 { *self.0 } }",
        ),
        (
            "struct S; impl<'a> From<&'a str> for S { fn from(s: &str) -> S { S } }",
            "struct S; impl From<&str> for S { fn from(s: &str) -> S { S } }",
        ),
        (
            "trait T {} impl<'a> T for &'a u8 {}",
            "trait T {} impl T for &u8 {}",
        ),
        (
            "struct S<'a, U>(&'a U); impl<'a, U: Clone> S<'a, U> {}",
            "struct S<'a, U>(&'a U); impl<U: Clone> S<'_, U> {}",
        ),
        (
            "struct S<'a, 'b>(&'a u8, &'b u8); impl<'a> S<'a, 'static> {}",
            "struct S<'a, 'b>(&'a u8, &'b u8); impl S<'_, 'static> {}",
        ),
        (
            "trait T {} impl<'a> T for (&'a u8, &u16) {}",
            "trait T {} impl T for (&u8, &u16) {}",
        ),
    ] {
        equivalent(named, anonymous);
    }
}

#[test]
fn impl_lifetimes_named_beyond_one_header_position_stay() {
    for (named, anonymous) in [
        (
            "struct S<'a>(&'a u8); impl<'a> S<'a> { fn get(&self) -> &'a u8 { self.0 } }",
            "struct S<'a>(&'a u8); impl S<'_> { fn get(&self) -> &u8 { self.0 } }",
        ),
        (
            "struct S<'a, 'b>(&'a u8, &'b u8); impl<'a> S<'a, 'a> {}",
            "struct S<'a, 'b>(&'a u8, &'b u8); impl S<'_, '_> {}",
        ),
        (
            "struct S<'a, U>(&'a U); impl<'a, U: 'a> S<'a, U> {}",
            "struct S<'a, U>(&'a U); impl<U> S<'_, U> {}",
        ),
        (
            "struct S<'a, U>(&'a U); impl<'a, U> S<'a, U> where U: 'a {}",
            "struct S<'a, U>(&'a U); impl<U> S<'_, U> {}",
        ),
        (
            "trait T {} impl<'a> T for fn(&'a u8) {}",
            "trait T {} impl T for fn(&u8) {}",
        ),
        (
            "trait T {} impl<'a> T for Box<dyn Send + 'a> {}",
            "trait T {} impl T for Box<dyn Send + '_> {}",
        ),
        (
            "struct S<'a>(&'a u8); impl<'a> S<'a> { fn f() { m!('a); } }",
            "struct S<'a>(&'a u8); impl S<'_> { fn f() { m!('a); } }",
        ),
        (
            "struct S<'a>(&'a u8); impl<#[cfg(all())] 'a> S<'a> {}",
            "struct S<'a>(&'a u8); impl S<'_> {}",
        ),
        (
            "struct S<'a, const N: usize>(&'a u8); impl<'a> S<'a, { m!() }> {}",
            "struct S<'a, const N: usize>(&'a u8); impl S<'_, { m!() }> {}",
        ),
        (
            "trait T {} impl<'a> T for (&'a u8, m!()) {}",
            "trait T {} impl T for (&u8, m!()) {}",
        ),
    ] {
        separate(named, anonymous);
    }
}
