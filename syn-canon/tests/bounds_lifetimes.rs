//! Trait bound order and lifetimes the elision rules assign exactly.

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

#[test]
fn bounds_of_one_type_match_in_any_order() {
    for (written, sorted) in [
        (
            "fn f<T>(t: T) -> T where T: Copy + Clone { t }",
            "fn f<T>(t: T) -> T where T: Clone + Copy { t }",
        ),
        (
            "fn f<T: Copy + Clone>(t: T) -> T { t }",
            "fn f<T>(t: T) -> T where T: Clone + Copy { t }",
        ),
        (
            "fn f<T: Copy>(t: T) -> T where T: Clone { t }",
            "fn f<T>(t: T) -> T where T: Clone + Copy { t }",
        ),
        (
            "fn f<T, U>(t: T, u: U) where T: AsRef<U> + Clone + 'static, U: ?Sized + Send {}",
            "fn f<T, U>(t: T, u: U) where T: 'static + Clone + AsRef<U>, U: Send + ?Sized {}",
        ),
        (
            "struct S<T: Send + Sync>(T);",
            "struct S<T>(T) where T: Sync + Send;",
        ),
        (
            "trait Tr {} struct S<T>(T); impl<T: Default + Clone> Tr for S<T> {}",
            "trait Tr {} struct S<T>(T); impl<T> Tr for S<T> where T: Clone + Default {}",
        ),
    ] {
        equivalent(written, sorted);
    }
    equivalent(
        "fn f<A: Into<A> + Clone>(a: A) {}",
        "fn f<B: Clone + Into<B>>(b: B) {}",
    );
}

#[test]
fn function_and_binding_bounds_keep_their_order() {
    for (written, reordered) in [
        (
            "fn f<F>(f: F) where F: Fn(u8) -> u8 + Clone {}",
            "fn f<F>(f: F) where F: Clone + Fn(u8) -> u8 {}",
        ),
        (
            "fn f<I>(i: I) where I: Iterator<Item = u8> + Clone {}",
            "fn f<I>(i: I) where I: Clone + Iterator<Item = u8> {}",
        ),
        (
            "fn f<T>(t: T) where for<'a> T: Copy + Clone {}",
            "fn f<T>(t: T) where for<'a> T: Clone + Copy {}",
        ),
    ] {
        separate(written, reordered);
    }
    separate(
        "#[observe] fn f<T>(t: T) where T: Copy + Clone {}",
        "#[observe] fn f<T>(t: T) where T: Clone + Copy {}",
    );
}

#[test]
fn exactly_elidable_lifetimes_match_their_elided_form() {
    for (named, elided) in [
        (
            "fn f<'a>(x: &'a u32) -> &'a u32 { x }",
            "fn f(x: &u32) -> &u32 { x }",
        ),
        ("fn f<'a>(x: &'a str) {}", "fn f(x: &str) {}"),
        (
            "fn f<'a>(x: &'a [u8], i: usize) -> &'a u8 { &x[i] }",
            "fn f(x: &[u8], i: usize) -> &u8 { &x[i] }",
        ),
        (
            "fn f<'a, T>(x: &'a mut Vec<T>) -> &'a mut T { &mut x[0] }",
            "fn f<T>(x: &mut Vec<T>) -> &mut T { &mut x[0] }",
        ),
        (
            "fn f<'a>(x: &'a u8, y: &u8) -> u8 { *x + *y }",
            "fn f(x: &u8, y: &u8) -> u8 { *x + *y }",
        ),
        (
            "struct S(u8); impl S { fn get<'a>(&'a self) -> &'a u8 { &self.0 } }",
            "struct S(u8); impl S { fn get(&self) -> &u8 { &self.0 } }",
        ),
        (
            "struct S(u8); impl S { fn get<'a>(&'a self, x: &u8) -> &'a u8 { &self.0 } }",
            "struct S(u8); impl S { fn get(&self, x: &u8) -> &u8 { &self.0 } }",
        ),
        (
            "trait Tr { fn get<'a>(&'a self) -> &'a u8; }",
            "trait Tr { fn get(&self) -> &u8; }",
        ),
    ] {
        equivalent(named, elided);
    }
}

#[test]
fn lifetimes_elision_would_reassign_stay_named() {
    for (named, elided) in [
        (
            "fn f<'a>(x: &'a u8, y: &'a u8) -> &'a u8 { x }",
            "fn f(x: &u8, y: &u8) -> &u8 { x }",
        ),
        (
            "fn f<'a>(x: &'a u8) -> &'a u8 where 'a: 'a { x }",
            "fn f(x: &u8) -> &u8 { x }",
        ),
        (
            "fn f<'a, T: 'a>(x: &'a T) -> &'a T { x }",
            "fn f<T>(x: &T) -> &T { x }",
        ),
        (
            "fn f<'a>(x: &'a u8) -> &'a u8 { let y: &'a u8 = x; y }",
            "fn f(x: &u8) -> &u8 { let y: &u8 = x; y }",
        ),
        (
            "fn f<'a>(x: &'a u8) -> &'a u8 { m!('a); x }",
            "fn f(x: &u8) -> &u8 { m!('a); x }",
        ),
        (
            "fn f<'a>(x: &'a u8, y: &u8) -> &'a u8 { x }",
            "fn f(x: &u8, y: &u8) -> &u8 { x }",
        ),
        (
            "struct S(u8); impl S { fn get<'a>(&self, x: &'a u8) -> &'a u8 { x } }",
            "struct S(u8); impl S { fn get(&self, x: &u8) -> &u8 { x } }",
        ),
        (
            "fn f<'a>(x: Wrap<'a>) -> &'a u8 { x.0 }",
            "fn f(x: Wrap) -> &u8 { x.0 }",
        ),
        ("fn f<'a>(g: fn(&'a u8)) {}", "fn f(g: fn(&u8)) {}"),
        (
            "fn f<'a>(x: &'a u8) -> Box<dyn Fn() -> &'a u8> { loop {} }",
            "fn f(x: &u8) -> Box<dyn Fn() -> &u8> { loop {} }",
        ),
        (
            "fn f<'a>(x: &'a u8) -> impl Sized + 'a { x }",
            "fn f(x: &u8) -> impl Sized { x }",
        ),
    ] {
        separate(named, elided);
    }
    separate(
        "#[observe] fn f<'a>(x: &'a u32) -> &'a u32 { x }",
        "#[observe] fn f(x: &u32) -> &u32 { x }",
    );
}

#[test]
fn function_views_sort_bounds_and_elide_lifetimes() {
    let file = parse(
        "fn first<'a, T>(x: &'a T) -> &'a T where T: Copy + Clone { x } fn second<U: Clone + Copy>(y: &U) -> &U { y }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
}

#[test]
fn receivers_of_every_kind_follow_the_elision_rules() {
    for (named, elided) in [
        (
            "fn get<'a>(self, x: &'a u8) -> &'a u8 { x }",
            "fn get(self, x: &u8) -> &u8 { x }",
        ),
        (
            "fn get<'a>(self: &'a Self) -> &'a u8 { &self.0 }",
            "fn get(self: &Self) -> &u8 { &self.0 }",
        ),
        (
            "fn get<'a>(self: Self, x: &'a u8) -> &'a u8 { x }",
            "fn get(self: Self, x: &u8) -> &u8 { x }",
        ),
    ] {
        equivalent(
            &format!("struct S(u8); impl S {{ {named} }}"),
            &format!("struct S(u8); impl S {{ {elided} }}"),
        );
    }
    for (named, elided) in [
        (
            "fn get<'a>(self: Box<Self>, x: &'a u8) -> &'a u8 { x }",
            "fn get(self: Box<Self>, x: &u8) -> &u8 { x }",
        ),
        (
            "fn get<'a>(#[inspect] &'a self) -> &'a u8 { &self.0 }",
            "fn get(#[inspect] &self) -> &u8 { &self.0 }",
        ),
    ] {
        separate(
            &format!("struct S(u8); impl S {{ {named} }}"),
            &format!("struct S(u8); impl S {{ {elided} }}"),
        );
    }
}

#[test]
fn opaque_signatures_and_bodies_keep_the_lifetime() {
    for (named, elided) in [
        (
            "fn f<'a>(#[inspect] x: &'a u8) -> &'a u8 { x }",
            "fn f(#[inspect] x: &u8) -> &u8 { x }",
        ),
        (
            "fn f<'a>(x: &'a m!()) -> &'a u8 { x }",
            "fn f(x: &m!()) -> &u8 { x }",
        ),
        (
            "fn f<'a>(x: &'a [u8; m!()]) -> &'a u8 { &x[0] }",
            "fn f(x: &[u8; m!()]) -> &u8 { &x[0] }",
        ),
        (
            "fn f<'a>(x: &'a u8) -> &'a u8 { #[inspect('a)] let y = x; y }",
            "fn f(x: &u8) -> &u8 { #[inspect('a)] let y = x; y }",
        ),
    ] {
        separate(named, elided);
    }
}

#[test]
fn bounds_with_grouped_and_literal_arguments_sort() {
    for (written, sorted) in [
        (
            "fn f<T>(t: T) where T: AsRef<[u8]> + Clone {}",
            "fn f<T>(t: T) where T: Clone + AsRef<[u8]> {}",
        ),
        (
            "fn f<T>(t: T) where T: Tr<{ 1 }> + Clone {}",
            "fn f<T>(t: T) where T: Clone + Tr<{ 1 }> {}",
        ),
        (
            "fn f<T>(t: T) where T: Tr<1> + Clone {}",
            "fn f<T>(t: T) where T: Clone + Tr<1> {}",
        ),
    ] {
        equivalent(written, sorted);
    }
}
