//! Accessor synonyms over bindings of proven standard collection types read as one form.

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

/// Each `(params, ret, read, written spellings)` row matches every spelling with `read`.
fn rows(rows: &[(&str, &str, &str, &[&str])]) {
    for (params, ret, read, spellings) in rows {
        for written in *spellings {
            equivalent(
                &function(params, ret, written),
                &function(params, ret, read),
            );
        }
    }
}

#[test]
fn whole_views_read_as_the_inherent_view() {
    rows(&[
        (
            "s: String",
            "usize",
            "count(s.as_str())",
            &["count(&s[..])", "count(&*s)"],
        ),
        ("s: &String", "&str", "s.as_str()", &["&s[..]"]),
        ("s: &mut String", "&str", "s.as_str()", &["&s[..]"]),
        (
            "v: Vec<u32>",
            "u32",
            "sum(v.as_slice())",
            &["sum(&v[..])", "sum(&*v)"],
        ),
        ("v: &Vec<u32>", "&[u32]", "v.as_slice()", &["&v[..]"]),
    ]);
}

#[test]
fn empty_string_comparisons_read_as_emptiness() {
    for params in ["s: &str", "s: String", "s: &String"] {
        rows(&[
            (
                params,
                "bool",
                "s.is_empty()",
                &["s == \"\"", "\"\" == s", "s.len() == 0"],
            ),
            (params, "bool", "!s.is_empty()", &["s != \"\"", "\"\" != s"]),
        ]);
    }
}

#[test]
fn first_elements_counts_and_byte_lengths_read_as_one_form() {
    for params in ["v: &[u32]", "v: &Vec<u32>", "v: &mut Vec<u32>"] {
        rows(&[
            (
                params,
                "Option<&u32>",
                "v.first()",
                &["v.get(0)", "v.get(0usize)", "v.iter().next()"],
            ),
            (params, "usize", "v.len()", &["v.iter().count()"]),
            (params, "bool", "v.is_empty()", &["v.iter().count() == 0"]),
        ]);
    }
    rows(&[
        (
            "a: [u32; 3]",
            "Option<u32>",
            "a.first().copied()",
            &["a.get(0).copied()", "a.iter().next().copied()"],
        ),
        ("v: Vec<u32>", "usize", "v.len()", &["v.iter().count()"]),
    ]);
    for params in ["s: &str", "s: String", "s: &String"] {
        rows(&[(params, "usize", "s.len()", &["s.as_bytes().len()"])]);
    }
    equivalent(
        "fn f() -> usize { let w: Vec<u32> = make(); w.iter().count() }",
        "fn f() -> usize { let w: Vec<u32> = make(); w.len() }",
    );
}

#[test]
fn unproven_receivers_and_other_accessors_stay_apart() {
    for (left, right) in [
        (
            function("v: Bag", "usize", "v.iter().count()"),
            function("v: Bag", "usize", "v.len()"),
        ),
        (
            function("s: String", "usize", "count(&s)"),
            function("s: String", "usize", "count(s.as_str())"),
        ),
        (
            function("s: &String", "usize", "count(&*s)"),
            function("s: &String", "usize", "count(s.as_str())"),
        ),
        (
            function("v: &[u32]", "Option<&u32>", "v.get(1)"),
            function("v: &[u32]", "Option<&u32>", "v.first()"),
        ),
        (
            function("v: &[u32]", "u32", "sum(&v[..])"),
            function("v: &[u32]", "u32", "sum(v.as_slice())"),
        ),
        (
            function("s: Text", "usize", "s.as_bytes().len()"),
            function("s: Text", "usize", "s.len()"),
        ),
        (
            function("s: &str", "bool", "s == \" \""),
            function("s: &str", "bool", "s.is_empty()"),
        ),
        (
            "trait Store { fn get(&self, index: usize) -> Option<&u32>; }\nfn f(v: &Vec<u32>) -> Option<&u32> { v.get(0) }".to_owned(),
            "trait Store { fn get(&self, index: usize) -> Option<&u32>; }\nfn f(v: &Vec<u32>) -> Option<&u32> { v.first() }".to_owned(),
        ),
    ] {
        separate(&left, &right);
    }
}

#[test]
fn accessor_traits_leave_the_collection_readings_running() {
    equivalent(
        "trait Store { fn get(&self, index: usize) -> Option<&u32>; }\nfn f(v: &Vec<u32>) -> bool { v.len() == 0 }",
        "trait Store { fn get(&self, index: usize) -> Option<&u32>; }\nfn f(v: &Vec<u32>) -> bool { v.is_empty() }",
    );
}
