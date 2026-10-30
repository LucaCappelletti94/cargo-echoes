//! Predicates and conversions of bindings declared as the prelude `Option` or `Result` read
//! as the `match` they perform.

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

#[test]
fn option_predicates_read_as_one_form() {
    let option = "o: Option<u32>";
    for group in [
        &[
            "o.is_some()",
            "!o.is_none()",
            "o != None",
            "None != o",
            "matches!(o, Some(_))",
        ][..],
        &[
            "o.is_none()",
            "!o.is_some()",
            "o == None",
            "matches!(o, None)",
            "match o { Some(_) => false, _ => true }",
        ][..],
    ] {
        for written in &group[1..] {
            equivalent(
                &function(option, "bool", written),
                &function(option, "bool", group[0]),
            );
        }
    }
    equivalent(
        &function("o: &Option<String>", "bool", "o.is_some()"),
        &function("o: &Option<String>", "bool", "matches!(o, Some(_))"),
    );
    equivalent(
        "fn f(v: u32) -> bool { let o: Option<u32> = Some(v); o.is_none() }",
        "fn f(v: u32) -> bool { let o: Option<u32> = Some(v); o == None }",
    );
}

#[test]
fn result_predicates_read_as_one_form() {
    let result = "r: Result<u32, u8>";
    for group in [
        &["r.is_ok()", "!r.is_err()", "matches!(r, Ok(_))"][..],
        &["r.is_err()", "!r.is_ok()", "matches!(r, Err(_))"][..],
    ] {
        for written in &group[1..] {
            equivalent(
                &function(result, "bool", written),
                &function(result, "bool", group[0]),
            );
        }
    }
}

#[test]
fn qualified_variants_read_as_prelude_names() {
    for (written, read) in [
        (
            function("x: u32", "Option<u32>", "Option::Some(x)"),
            function("x: u32", "Option<u32>", "Some(x)"),
        ),
        (
            function("", "Option<u32>", "Option::None"),
            function("", "Option<u32>", "None"),
        ),
        (
            function("x: u32", "Result<u32, u8>", "Result::Ok(x)"),
            function("x: u32", "Result<u32, u8>", "Ok(x)"),
        ),
        (
            function("e: u8", "Result<u32, u8>", "Result::Err(e)"),
            function("e: u8", "Result<u32, u8>", "Err(e)"),
        ),
        (
            function(
                "o: Option<u32>",
                "u32",
                "match o { Option::Some(v) => v, Option::None => 0 }",
            ),
            function(
                "o: Option<u32>",
                "u32",
                "match o { Some(v) => v, None => 0 }",
            ),
        ),
    ] {
        equivalent(&written, &read);
    }
}

#[test]
fn defaults_identities_and_takes_read_as_their_match() {
    for (written, read) in [
        (
            function("o: Option<u32>", "u32", "o.unwrap_or(0)"),
            function(
                "o: Option<u32>",
                "u32",
                "match o { Some(v) => v, None => 0 }",
            ),
        ),
        (
            function("o: Option<u32>", "u32", "o.unwrap_or_else(|| 7)"),
            function("o: Option<u32>", "u32", "o.unwrap_or(7)"),
        ),
        (
            function("o: Option<u32>, d: u32", "u32", "o.unwrap_or(d)"),
            function(
                "o: Option<u32>, d: u32",
                "u32",
                "match o { Some(v) => v, None => d }",
            ),
        ),
        (
            function(
                "o: Option<String>",
                "Option<String>",
                "match o { Some(v) => Some(v), None => None }",
            ),
            function("o: Option<String>", "Option<String>", "o"),
        ),
        (
            function(
                "o: Option<String>",
                "Option<String>",
                "match o { Some(v) => Some(v), _ => None }",
            ),
            function("o: Option<String>", "Option<String>", "o"),
        ),
        (
            function(
                "o: &mut Option<String>",
                "Option<String>",
                "std::mem::replace(o, None)",
            ),
            function("o: &mut Option<String>", "Option<String>", "o.take()"),
        ),
    ] {
        equivalent(&written, &read);
    }
}

#[test]
fn unproven_receivers_and_moving_conversions_stay_apart() {
    for (left, right) in [
        (
            function("o: Maybe", "bool", "o.is_some()"),
            function("o: Maybe", "bool", "matches!(o, Some(_))"),
        ),
        (
            "struct Option<T>(T);\nimpl<T> Option<T> { fn is_some(&self) -> bool { false } }\nfn f(o: Option<u32>) -> bool { o.is_some() }".to_owned(),
            "struct Option<T>(T);\nimpl<T> Option<T> { fn is_some(&self) -> bool { false } }\nfn f(o: Option<u32>) -> bool { matches!(o, Some(_)) }".to_owned(),
        ),
        (
            function("r: Result<u32, u8>", "Option<u32>", "r.ok()"),
            function("r: Result<u32, u8>", "Option<u32>", "match r { Ok(v) => Some(v), Err(_) => None }"),
        ),
        (
            function("o: Option<u32>, e: u8", "Result<u32, u8>", "o.ok_or(e)"),
            function("o: Option<u32>, e: u8", "Result<u32, u8>", "match o { Some(v) => Ok(v), None => Err(e) }"),
        ),
        (
            function("o: Option<String>", "String", "o.unwrap_or(make())"),
            function("o: Option<String>", "String", "match o { Some(v) => v, None => make() }"),
        ),
        (
            function("o: &Option<String>", "Option<String>", "match o { Some(v) => Some(v), None => None }"),
            function("o: &Option<String>", "Option<String>", "o"),
        ),
        (
            function("o: Option<String>", "Option<String>", "match o { _ => None, Some(v) => Some(v) }"),
            function("o: Option<String>", "Option<String>", "o"),
        ),
        (
            function("o: &mut Option<String>, g: fn(&mut Option<String>, Option<String>) -> Option<String>", "Option<String>", "(g)(o, None)"),
            function("o: &mut Option<String>, g: fn(&mut Option<String>, Option<String>) -> Option<String>", "Option<String>", "o.take()"),
        ),
    ] {
        separate(&left, &right);
    }
}
