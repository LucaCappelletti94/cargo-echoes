//! Conversions and defaults whose target a declaration fixes read as their standard spelling.

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

fn function(params: &str, ret: &str, body: &str) -> String {
    format!("fn f({params}) -> {ret} {{ {body} }}")
}

fn rows(params: &str, ret: &str, groups: &[&[&str]]) {
    for group in groups {
        for written in &group[1..] {
            equivalent(
                &function(params, ret, written),
                &function(params, ret, group[0]),
            );
        }
    }
}

#[test]
fn declared_primitive_conversions_read_as_from() {
    rows(
        "x: u16",
        "u32",
        &[
            &["u32::from(x)", "x.into()", "x as u32"],
            &["let y: u32 = u32::from(x); y", "let y: u32 = x.into(); y"],
        ],
    );
    rows("x: u32", "u32", &[&["x", "x.into()"]]);
    rows("b: bool", "u8", &[&["b as u8", "b.into()"]]);
    rows("c: char", "u32", &[&["c as u32", "c.into()"]]);
    rows("b: u8", "char", &[&["b as char", "b.into()"]]);
    rows("", "u32", &[&["u32::from('a')", "'a'.into()"]]);
    rows("", "u8", &[&["u8::from(true)", "true.into()"]]);
    rows("", "u32", &[&["u32::from(7u16)", "7u16.into()"]]);
}

#[test]
fn declared_string_conversions_read_as_from() {
    rows(
        "",
        "String",
        &[
            &["\"a\".to_string()", "\"a\".into()", "String::from(\"a\")"],
            &[
                "let s: String = String::from(\"a\"); s",
                "let s: String = \"a\".into(); s",
            ],
        ],
    );
    rows("t: &str", "String", &[&["t.to_string()", "t.into()"]]);
    rows("c: char", "String", &[&["String::from(c)", "c.into()"]]);
    rows("t: String", "String", &[&["String::from(t)", "t.into()"]]);
}

#[test]
fn standard_defaults_read_as_their_values() {
    rows("", "u32", &[&["0", "u32::default()", "Default::default()"]]);
    rows("", "&'static str", &[&["\"\"", "Default::default()"]]);
    rows(
        "",
        "i64",
        &[&[
            "let x: i64 = 0; x",
            "let x: i64 = Default::default(); x",
            "let x: i64 = i64::default(); x",
        ]],
    );
    rows(
        "",
        "bool",
        &[&["false", "bool::default()", "Default::default()"]],
    );
    rows(
        "",
        "char",
        &[&["'\\0'", "char::default()", "Default::default()"]],
    );
    rows(
        "",
        "String",
        &[&["String::new()", "String::default()", "Default::default()"]],
    );
    rows(
        "",
        "Vec<u32>",
        &[
            &["Vec::new()", "Vec::default()", "Default::default()"],
            &["Vec::<u32>::new()", "Vec::<u32>::default()"],
        ],
    );
}

#[test]
fn replacing_with_a_default_reads_as_take() {
    rows(
        "v: &mut Vec<u32>",
        "Vec<u32>",
        &[&[
            "std::mem::take(v)",
            "std::mem::replace(v, Default::default())",
            "std::mem::replace(v, Vec::new())",
            "std::mem::replace(v, Vec::default())",
        ]],
    );
    rows(
        "s: &mut String",
        "String",
        &[&["core::mem::take(s)", "core::mem::replace(s, String::new())"]],
    );
    rows(
        "x: &mut u32",
        "u32",
        &[&["std::mem::take(x)", "std::mem::replace(x, u32::default())"]],
    );
}

#[test]
fn undeclared_targets_lossy_conversions_and_user_defaults_stay_apart() {
    for (left, right) in [
        (
            "fn f(x: u16) { g(x.into()) }".to_owned(),
            "fn f(x: u16) { g(u32::from(x)) }".to_owned(),
        ),
        (
            function("x: u32", "u16", "x.into()"),
            function("x: u32", "u16", "x as u16"),
        ),
        (
            function("x: Num", "u32", "x.into()"),
            function("x: Num", "u32", "u32::from(x)"),
        ),
        (
            "fn f() { g(Default::default()) }".to_owned(),
            "fn f() { g(0) }".to_owned(),
        ),
        (
            "trait Default { fn default() -> Self; }\nfn f() -> u32 { u32::default() }".to_owned(),
            "trait Default { fn default() -> Self; }\nfn f() -> u32 { 0 }".to_owned(),
        ),
        (
            "use std::collections::HashMap;\nfn f() -> HashMap<u32, u32> { HashMap::default() }".to_owned(),
            "use std::collections::HashMap;\nfn f() -> HashMap<u32, u32> { HashMap::new() }".to_owned(),
        ),
        (
            function("v: &mut Vec<u32>", "Vec<u32>", "std::mem::replace(v, vec![1])"),
            function("v: &mut Vec<u32>", "Vec<u32>", "std::mem::take(v)"),
        ),
        (
            function("v: &mut Vec<u32>", "Vec<u32>", "std::mem::replace(v, (make)())"),
            function("v: &mut Vec<u32>", "Vec<u32>", "std::mem::take(v)"),
        ),
        (
            function("", "u32", "5char.into()"),
            function("", "u32", "u32::from(5char)"),
        ),
        (
            function("", "u32", "1.5.into()"),
            function("", "u32", "u32::from(1.5)"),
        ),
        (
            function("v: &[u8]", "u64", "v.len().into()"),
            function("v: &[u8]", "u64", "v.len() as u64"),
        ),
        (
            "struct Num;\nimpl Num { fn new() -> Num { Num } }\nfn f(v: &mut Num) -> Num { std::mem::replace(v, Num::new()) }".to_owned(),
            "struct Num;\nimpl Num { fn new() -> Num { Num } }\nfn f(v: &mut Num) -> Num { std::mem::take(v) }".to_owned(),
        ),
    ] {
        separate(&left, &right);
    }
}
