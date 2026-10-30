//! Owned copies of bindings declared as `&str`, `String` or `&String` read as one form.

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

fn function(params: &str, body: &str) -> String {
    format!("fn f({params}) -> String {{ {body} }}")
}

#[test]
fn str_copies_read_as_one_form() {
    for written in ["s.to_owned()", "String::from(s)"] {
        equivalent(
            &function("s: &str", written),
            &function("s: &str", "s.to_string()"),
        );
    }
    equivalent(
        "fn f(v: &'static str) -> String { let s: &str = v; s.to_owned() }",
        "fn f(v: &'static str) -> String { let s: &str = v; String::from(s) }",
    );
    equivalent(
        "fn f() -> impl Fn(&str) -> String { |s: &str| s.to_owned() }",
        "fn f() -> impl Fn(&str) -> String { |s: &str| s.to_string() }",
    );
}

#[test]
fn string_copies_read_as_one_form() {
    for params in ["s: String", "s: &String"] {
        for written in ["s.to_owned()", "s.clone()", "String::from(s.as_str())"] {
            equivalent(
                &function(params, written),
                &function(params, "s.to_string()"),
            );
        }
    }
    equivalent(
        "fn f(v: String) -> String { let s: String = v; s.clone() }",
        "fn f(v: String) -> String { let s: String = v; String::from(s.as_str()) }",
    );
}

#[test]
fn unproven_and_differently_allocating_copies_stay_apart() {
    for (left, right) in [
        (
            function("s: Name", "s.to_owned()"),
            function("s: Name", "s.to_string()"),
        ),
        (
            function("s: &str", "format!(\"{s}\")"),
            function("s: &str", "s.to_string()"),
        ),
        (
            function("s: &str", "s.into()"),
            function("s: &str", "s.to_string()"),
        ),
        (
            function("s: &str", "s.clone().to_string()"),
            function("s: &str", "s.to_string().clone()"),
        ),
        (
            "struct String;\nfn f(s: String) -> String { s.clone() }".to_owned(),
            "struct String;\nfn f(s: String) -> String { s.to_owned() }".to_owned(),
        ),
        (
            "trait Text { fn as_str(self) -> &'static str; }\nimpl Text for String { fn as_str(self) -> &'static str { \"other\" } }\nfn f(s: String) -> String { String::from(s.as_str()) }".to_owned(),
            "trait Text { fn as_str(self) -> &'static str; }\nimpl Text for String { fn as_str(self) -> &'static str { \"other\" } }\nfn f(s: String) -> String { s.clone() }".to_owned(),
        ),
        (
            "trait Copied { fn clone(self) -> String; }\nimpl Copied for String { fn clone(self) -> String { String::new() } }\nfn f(s: String) -> String { s.clone() }".to_owned(),
            "trait Copied { fn clone(self) -> String; }\nimpl Copied for String { fn clone(self) -> String { String::new() } }\nfn f(s: String) -> String { s.to_owned() }".to_owned(),
        ),
    ] {
        separate(&left, &right);
    }
}
