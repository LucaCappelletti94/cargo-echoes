//! Annotations of `let` bindings whose initializer fixes the same type read as the initializer.

#[path = "support/view.rs"]
mod view;

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

fn function(prefix: &str, binding: &str, rest: &str) -> String {
    format!("{prefix}fn f() -> usize {{ {binding}; {rest} }}")
}

#[test]
fn collection_annotations_read_as_turbofish() {
    for (prefix, annotated, turbofish, rest) in [
        (
            "",
            "let v: Vec<u32> = Vec::new()",
            "let v = Vec::<u32>::new()",
            "v.len()",
        ),
        (
            "",
            "let mut v: Vec<u32> = Vec::new()",
            "let mut v = Vec::<u32>::new()",
            "v.push(1); v.len()",
        ),
        (
            "",
            "let v: Vec<u8> = Vec::with_capacity(4)",
            "let v = Vec::<u8>::with_capacity(4)",
            "v.capacity()",
        ),
        (
            "",
            "let s: String = String::new()",
            "let s = String::new()",
            "s.len()",
        ),
        (
            "",
            "let s: String = String::with_capacity(8)",
            "let s = String::with_capacity(8)",
            "s.capacity()",
        ),
        (
            "use std::collections::HashMap;\n",
            "let m: HashMap<u32, u8> = HashMap::new()",
            "let m = HashMap::<u32, u8>::new()",
            "m.len()",
        ),
        (
            "use std::collections::HashSet;\n",
            "let s: HashSet<u32> = HashSet::with_capacity(2)",
            "let s = HashSet::<u32>::with_capacity(2)",
            "s.len()",
        ),
        (
            "use std::collections::VecDeque;\n",
            "let q: VecDeque<u32> = VecDeque::new()",
            "let q = VecDeque::<u32>::new()",
            "q.len()",
        ),
        (
            "use std::collections::btree_map::BTreeMap;\n",
            "let m: BTreeMap<u32, u8> = BTreeMap::new()",
            "let m = BTreeMap::<u32, u8>::new()",
            "m.len()",
        ),
        (
            "",
            "let s: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new()",
            "let s = std::collections::BTreeSet::<u32>::new()",
            "s.len()",
        ),
        (
            "",
            "let s: std::string::String = std::string::String::new()",
            "let s = std::string::String::new()",
            "s.len()",
        ),
        (
            "extern crate alloc;\n",
            "let v: alloc::vec::Vec<u8> = alloc::vec::Vec::new()",
            "let v = alloc::vec::Vec::<u8>::new()",
            "v.len()",
        ),
    ] {
        equivalent(
            &function(prefix, annotated, rest),
            &function(prefix, turbofish, rest),
        );
    }
}

#[test]
fn imports_the_function_view_keeps_bare_still_read_as_standard() {
    let file = |binding: &str| {
        parse(&format!(
            "use std::collections::HashMap;\nfn f() -> usize {{ #[derive(Clone)] struct Local; {binding}; m.len() }}"
        ))
    };
    assert_eq!(
        view::view_of(&file("let m: HashMap<u32, u8> = HashMap::new()"), "f"),
        view::view_of(&file("let m = HashMap::<u32, u8>::new()"), "f"),
    );
}

#[test]
fn typed_values_and_constructors_prove_their_bindings() {
    for (written, read) in [
        (
            "fn f(x: u32) -> u32 { let y: u32 = x; y + 1 }",
            "fn f(x: u32) -> u32 { let y = x; y + 1 }",
        ),
        (
            "fn f(c: char) -> bool { let d: char = c; d == 'a' }",
            "fn f(c: char) -> bool { let d = c; d == 'a' }",
        ),
        (
            "fn f() -> bool { let d: char = 'q'; d == 'a' }",
            "fn f() -> bool { let d = 'q'; d == 'a' }",
        ),
        (
            "fn f(b: bool) -> bool { let mut c: bool = b; c = !c; c }",
            "fn f(b: bool) -> bool { let mut c = b; c = !c; c }",
        ),
        (
            "fn f() { let v = Vec::new(); for x in v.iter() { g(x); } }",
            "fn f() { let v = Vec::new(); for x in &v { g(x); } }",
        ),
        (
            "fn f() -> String { let mut t = String::new(); t.push_str(\"a\"); t }",
            "fn f() -> String { let mut t = String::new(); t.push('a'); t }",
        ),
        (
            "fn f() -> usize { let s: ::std::vec::Vec<u8> = Vec::new(); s.len() }",
            "fn f() -> usize { let s = Vec::<u8>::new(); s.len() }",
        ),
    ] {
        equivalent(written, read);
    }
}

#[test]
fn inferred_coercing_and_user_annotations_stay_apart() {
    for (left, right) in [
        (
            function("", "let v: Vec<_> = Vec::new()", "v.len()"),
            function("", "let v = Vec::<_>::new()", "v.len()"),
        ),
        (
            function("", "let v: &[u32] = &Vec::new()", "v.len()"),
            function("", "let v = &Vec::<u32>::new()", "v.len()"),
        ),
        (
            function(
                "struct Vec<T>(Option<T>);\nimpl<T> Vec<T> { fn new() -> Self { Vec(None) } }\n",
                "let v: Vec<u32> = Vec::new()",
                "0",
            ),
            function(
                "struct Vec<T>(Option<T>);\nimpl<T> Vec<T> { fn new() -> Self { Vec(None) } }\n",
                "let v = Vec::<u32>::new()",
                "0",
            ),
        ),
        (
            function(
                "use other::HashMap;\n",
                "let m: HashMap<u32, u8> = HashMap::new()",
                "m.len()",
            ),
            function(
                "use other::HashMap;\n",
                "let m = HashMap::<u32, u8>::new()",
                "m.len()",
            ),
        ),
        (
            "fn f(x: Num) -> u32 { let y: u32 = x; y }".to_owned(),
            "fn f(x: Num) -> u32 { let y = x; y }".to_owned(),
        ),
        (
            function(
                "mod store { pub struct HashMap; }\nuse store::HashMap;\n",
                "let m: HashMap<u32, u8> = HashMap::new()",
                "0",
            ),
            function(
                "mod store { pub struct HashMap; }\nuse store::HashMap;\n",
                "let m = HashMap::<u32, u8>::new()",
                "0",
            ),
        ),
        (
            function("", "let v: Vec<u32> = (make)()", "v.len()"),
            function("", "let v = (make)()", "v.len()"),
        ),
        (
            function("", "let v: Vec<u32> = make()", "v.len()"),
            function("", "let v = make()", "v.len()"),
        ),
        (
            function("", "let v: Vec<u32> = Store::new()", "v.len()"),
            function("", "let v = Store::<u32>::new()", "v.len()"),
        ),
        (
            function(
                "",
                "let v: core::vec::Vec<u8> = core::vec::Vec::new()",
                "v.len()",
            ),
            function("", "let v = core::vec::Vec::<u8>::new()", "v.len()"),
        ),
        (
            function("", "let v: std::Vec<u8> = std::Vec::new()", "v.len()"),
            function("", "let v = std::Vec::<u8>::new()", "v.len()"),
        ),
    ] {
        separate(&left, &right);
    }
}
