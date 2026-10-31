//! Method synonyms over proven standard iterator chains read as one form.

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
fn copies_of_primitive_elements_read_as_copied() {
    rows(
        "v: Vec<u32>",
        "Vec<u32>",
        &[
            &[
                "v.iter().copied().collect()",
                "v.iter().map(|x| *x).collect()",
                "v.iter().cloned().collect()",
                "v.iter().map(|x| x.clone()).collect()",
            ],
            &[
                "v.iter().rev().skip(1).copied().collect()",
                "v.iter().rev().skip(1).map(|x| *x).collect()",
            ],
        ],
    );
    rows(
        "v: &[String]",
        "Vec<String>",
        &[&[
            "v.iter().cloned().collect()",
            "v.iter().map(|x| x.clone()).collect()",
        ]],
    );
}

#[test]
fn searches_read_as_find_next_and_contains() {
    rows(
        "v: &Vec<u32>",
        "Option<&u32>",
        &[&[
            "v.iter().find(|x| **x > 2)",
            "v.iter().filter(|x| **x > 2).next()",
        ]],
    );
    rows(
        "s: &str",
        "Option<char>",
        &[&["s.chars().next()", "s.chars().nth(0)"]],
    );
    rows(
        "s: String",
        "Option<u8>",
        &[&["s.bytes().next()", "s.bytes().nth(0)"]],
    );
    rows(
        "v: &[u32], y: u32",
        "bool",
        &[&[
            "v.contains(&y)",
            "v.iter().any(|x| *x == y)",
            "v.iter().any(|x| y == *x)",
        ]],
    );
    rows(
        "v: Vec<char>",
        "bool",
        &[&["v.contains(&'a')", "v.iter().any(|c| *c == 'a')"]],
    );
}

#[test]
fn negated_any_reads_as_all() {
    rows(
        "v: &[u32]",
        "bool",
        &[&["v.iter().all(|x| !(*x > 3))", "!v.iter().any(|x| *x > 3)"]],
    );
    rows(
        "s: &str",
        "bool",
        &[&[
            "s.chars().all(|c| !c.is_alphabetic())",
            "!s.chars().any(|c| c.is_alphabetic())",
        ]],
    );
}

#[test]
fn collect_annotations_read_as_turbofish() {
    rows(
        "v: &[u32]",
        "usize",
        &[&[
            "let w = v.iter().copied().collect::<Vec<u32>>(); w.len()",
            "let w: Vec<u32> = v.iter().copied().collect(); w.len()",
        ]],
    );
    rows(
        "v: &[u32]",
        "bool",
        &[&[
            "let w = v.iter().copied().collect::<Vec<u32>>(); w.is_empty()",
            "let w: Vec<u32> = v.iter().copied().collect(); w.len() == 0",
        ]],
    );
}

#[test]
fn unproven_iterators_and_other_shapes_stay_apart() {
    for (left, right) in [
        (
            function("it: Walker", "Vec<u32>", "it.map(|x| *x).collect()"),
            function("it: Walker", "Vec<u32>", "it.copied().collect()"),
        ),
        (
            function(
                "v: &[u32]",
                "bool",
                "!v.iter().any(|x| { if *x == 0 { return true; } *x > 3 })",
            ),
            function(
                "v: &[u32]",
                "bool",
                "v.iter().all(|x| !{ if *x == 0 { return true; } *x > 3 })",
            ),
        ),
        (
            function("v: &[String]", "Vec<String>", "v.iter().cloned().collect()"),
            function("v: &[String]", "Vec<String>", "v.iter().copied().collect()"),
        ),
        (
            function(
                "v: Vec<Item>",
                "Vec<Item>",
                "v.iter().map(|x| x.clone()).collect()",
            ),
            function("v: Vec<Item>", "Vec<Item>", "v.iter().cloned().collect()"),
        ),
        (
            function(
                "v: &[u32]",
                "usize",
                "let w: Vec<_> = v.iter().collect(); w.len()",
            ),
            function(
                "v: &[u32]",
                "usize",
                "let w = v.iter().collect::<Vec<_>>(); w.len()",
            ),
        ),
        (
            function("s: &str", "Option<char>", "s.chars().nth(1)"),
            function("s: &str", "Option<char>", "s.chars().next()"),
        ),
        (
            function("v: &[u32], y: Count", "bool", "v.iter().any(|x| *x == y)"),
            function("v: &[u32], y: Count", "bool", "v.contains(&y)"),
        ),
        (
            function(
                "v: &[u32]",
                "Option<u32>",
                "v.iter().map(|x| x + 1).filter(|x| *x > 2).next()",
            ),
            function(
                "v: &[u32]",
                "Option<u32>",
                "v.iter().filter(|x| **x > 1).map(|x| x + 1).next()",
            ),
        ),
    ] {
        separate(&left, &right);
    }
}
