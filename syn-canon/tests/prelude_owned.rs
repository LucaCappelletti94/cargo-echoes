//! Owned string literals and empty vectors under the standard prelude.

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

const OWNED: [&str; 3] = [
    "\"a\".to_string()",
    "\"a\".to_owned()",
    "String::from(\"a\")",
];

#[test]
fn owned_string_literals_match() {
    for template in [
        "fn f() -> String { VALUE }",
        "fn f() -> String { VALUE } fn main() { let s = f(); println!(\"{s}\"); }",
        "fn f(v: &mut Vec<String>) { v.push(VALUE); }",
        "fn f() -> usize { let s = VALUE; s.len() }",
    ] {
        for (index, left) in OWNED.iter().enumerate() {
            for right in &OWNED[index + 1..] {
                equivalent(
                    &template.replace("VALUE", left),
                    &template.replace("VALUE", right),
                );
            }
        }
    }
}

#[test]
fn empty_vectors_match() {
    for written in ["vec![]", "vec!()", "std::vec![]"] {
        equivalent(
            &format!("fn f() -> Vec<u32> {{ {written} }}"),
            "fn f() -> Vec<u32> { Vec::new() }",
        );
    }
}

#[test]
fn prelude_removals_keep_the_spelling() {
    for prefix in [
        "#![no_std]",
        "#![no_implicit_prelude]",
        "#![cfg_attr(not(feature = \"std\"), no_std)]",
        "use other::*;",
        "use custom::String;",
        "use custom::ToOwned;",
        "trait Owned { fn to_owned(&self) -> String; }",
    ] {
        separate(
            &format!("{prefix} fn f() -> String {{ \"a\".to_owned() }}"),
            &format!("{prefix} fn f() -> String {{ \"a\".to_string() }}"),
        );
    }
    separate(
        "use custom::String; fn f() -> String { String::from(\"a\") }",
        "use custom::String; fn f() -> String { \"a\".to_string() }",
    );
    separate(
        "#[no_implicit_prelude] mod m { fn f() -> ::std::string::String { \"a\".to_owned() } }",
        "#[no_implicit_prelude] mod m { fn f() -> ::std::string::String { \"a\".to_string() } }",
    );
    for prefix in [
        "#![no_std]",
        "struct Vec<T>(T); impl<T> Vec<T> { fn new() -> Self { loop {} } }",
        "use custom::Vec;",
    ] {
        separate(
            &format!("{prefix} fn f() -> Vec<u32> {{ vec![] }}"),
            &format!("{prefix} fn f() -> Vec<u32> {{ Vec::new() }}"),
        );
    }
}

#[test]
fn block_claims_keep_the_spelling() {
    for declaration in [
        "struct String;",
        "enum ToOwned {}",
        "union ToString { x: u8 }",
        "trait ToOwned {}",
        "trait ToString = Clone;",
        "type String = u8;",
        "mod ToOwned {}",
        "macro_rules! String { () => {} }",
        "extern crate custom as ToOwned;",
        "use custom::Text as String;",
        "use custom::*;",
        "fn g<ToOwned>() {}",
    ] {
        separate(
            &format!("fn f() -> usize {{ {declaration} \"a\".to_owned().len() }}"),
            &format!("fn f() -> usize {{ {declaration} \"a\".to_string().len() }}"),
        );
    }
    for declaration in ["macro_rules! vec { () => {} }", "struct Vec;"] {
        separate(
            &format!("fn f() -> usize {{ {declaration} let v: Vec<u8> = vec![]; v.len() }}"),
            &format!("fn f() -> usize {{ {declaration} let v: Vec<u8> = Vec::new(); v.len() }}"),
        );
    }
}

#[test]
fn attributed_syntax_keeps_the_spelling() {
    for (written, canonical) in [
        (
            "#[observe] let s = \"a\".to_owned(); s.len()",
            "#[observe] let s = \"a\".to_string(); s.len()",
        ),
        (
            "match 0u8 { #[observe] _ => \"a\".to_owned().len() }",
            "match 0u8 { #[observe] _ => \"a\".to_string().len() }",
        ),
        (
            "(#[observe] \"a\".to_owned()).len()",
            "(#[observe] \"a\".to_string()).len()",
        ),
    ] {
        separate(
            &format!("fn f() -> usize {{ {written} }}"),
            &format!("fn f() -> usize {{ {canonical} }}"),
        );
    }
    equivalent(
        "fn f() -> usize { let s = \"a\".to_owned(); match s.len() { n => n } }",
        "fn f() -> usize { let s = \"a\".to_string(); match s.len() { n => n } }",
    );
    equivalent(
        "fn f() -> String { fn g() -> String { \"b\".to_owned() } g() + \"a\" }",
        "fn f() -> String { fn g() -> String { String::from(\"b\") } g() + \"a\" }",
    );
}

#[test]
fn calls_of_other_callees_keep_their_literal() {
    equivalent(
        "fn f() -> usize { (|s: &str| s.len())(\"a\") + \"b\".to_owned().len() }",
        "fn f() -> usize { (|s: &str| s.len())(\"a\") + \"b\".to_string().len() }",
    );
}

#[test]
fn other_shapes_keep_the_spelling() {
    separate(
        "fn f(x: &str) -> String { \"a\".into() }",
        "fn f(x: &str) -> String { \"a\".to_string() }",
    );
    separate(
        "fn f() -> Vec<u8> { b\"a\".to_owned() }",
        "fn f() -> Vec<u8> { b\"a\".to_vec() }",
    );
    separate(
        "fn f() -> Vec<u32> { vec![1u32] }",
        "fn f() -> Vec<u32> { Vec::new() }",
    );
    separate(
        "#[observe] fn f() -> String { \"a\".to_owned() }",
        "#[observe] fn f() -> String { \"a\".to_string() }",
    );
}

#[test]
fn function_views_follow_the_crate_root_and_module_chain() {
    let file = parse(
        "fn first() -> String { \"a\".to_owned() } fn second() -> String { String::from(\"a\") }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "#![no_std] fn first() -> String { \"a\".to_owned() } fn second() -> String { String::from(\"a\") }",
    );
    assert_ne!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "trait Owned { fn to_owned(&self) -> String; } fn first() -> String { \"a\".to_owned() } fn second() -> String { \"a\".to_string() }",
    );
    assert_ne!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let root = parse("mod plain; mod bare;");
    let plain = parse(
        "pub fn first() -> String { \"a\".to_owned() } pub fn second() -> String { \"a\".to_string() }",
    );
    let bare = parse(
        "#![no_implicit_prelude] pub fn first() -> ::std::string::String { \"a\".to_owned() } pub fn second() -> ::std::string::String { \"a\".to_string() }",
    );
    let plain_path = [String::from("plain")];
    let bare_path = [String::from("bare")];
    let context = syn_canon::SourceContext::new(
        [
            (&[][..], &root),
            (&plain_path[..], &plain),
            (&bare_path[..], &bare),
        ],
        syn_canon::Edition::E2021,
    );
    let form = |file: &syn::File, index: usize| {
        let syn::Item::Fn(function) = &file.items[index] else {
            panic!("a function item");
        };
        context
            .function(&function.sig, &function.block)
            .expect("an indexed function")
            .canonicalize()
    };
    assert_eq!(form(&plain, 0), form(&plain, 1));
    assert_ne!(form(&bare, 0), form(&bare, 1));
}
