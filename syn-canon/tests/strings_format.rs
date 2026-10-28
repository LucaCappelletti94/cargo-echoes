//! Empty owned strings, argument-free `format!` and literal formatting arguments take one form.

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

fn separate(written: &str, read: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(read)),
            "{written}\n{read}"
        );
    }
}

fn owned(body: &str) -> String {
    format!("fn f() -> String {{ {body} }}")
}

fn printed(body: &str) -> String {
    format!("fn f(x: u32, out: &mut String) {{ {body} }}")
}

#[test]
fn empty_and_literal_strings_read_as_one_form() {
    for (written, read) in [
        ("\"\".to_string()", "String::new()"),
        ("\"\".to_owned()", "String::new()"),
        ("String::from(\"\")", "String::new()"),
        ("format!(\"abc\")", "\"abc\".to_string()"),
        ("format!(\"a{{b}}\")", "\"a{b}\".to_string()"),
        ("format!(\"\")", "String::new()"),
        ("format!(\"{}\", \"hi\")", "\"hi\".to_string()"),
        ("std::format!(\"ok\",)", "\"ok\".to_string()"),
    ] {
        equivalent(&owned(written), &owned(read));
    }
}

#[test]
fn literal_arguments_move_into_the_format_string() {
    for (written, read) in [
        ("println!(\"{}\", \"hi\");", "println!(\"hi\");"),
        (
            "println!(\"{} {}\", \"a{b\", x);",
            "println!(\"a{{b {x}\");",
        ),
        ("print!(\"<{}>\", \"}\");", "print!(\"<}}>\");"),
        ("eprintln!(\"{}{}\", x, \"!\");", "eprintln!(\"{x}!\");"),
        ("eprint!(\"{}\", \"e\");", "eprint!(\"e\");"),
        (
            "use std::fmt::Write; write!(out, \"{}\", \"w\").unwrap();",
            "use std::fmt::Write; write!(out, \"w\").unwrap();",
        ),
        (
            "use std::fmt::Write; writeln!(out, \"{}={}\", \"k\", x).unwrap();",
            "use std::fmt::Write; writeln!(out, \"k={x}\").unwrap();",
        ),
        (
            "println!(\"{:?}\", format!(\"{}\", \"n\"));",
            "println!(\"{:?}\", format!(\"n\"));",
        ),
    ] {
        equivalent(&printed(written), &printed(read));
    }
}

#[test]
fn specs_debug_and_shadowed_macros_keep_their_arguments() {
    for (written, read) in [
        ("format!(\"{x}\")", "x.to_string()"),
        ("format!(\"{:?}\", \"hi\")", "\"\\\"hi\\\"\".to_string()"),
        ("format!(\"{:>4}\", \"hi\")", "\"  hi\".to_string()"),
        ("format!(\"{0}\", \"hi\")", "\"hi\".to_string()"),
        ("format!(\"{}\", x)", "x.to_string()"),
        ("format!(\"abc\"x)", "\"abc\".to_string()"),
        ("\"a\".to_string()", "String::new()"),
    ] {
        separate(
            &format!("fn f(x: u32) -> String {{ {written} }}"),
            &format!("fn f(x: u32) -> String {{ {read} }}"),
        );
    }
    for (written, read) in [
        (
            "macro_rules! format { ($($t:tt)*) => { String::new() } } fn f() -> String { format!(\"abc\") }",
            "macro_rules! format { ($($t:tt)*) => { String::new() } } fn f() -> String { \"abc\".to_string() }",
        ),
        (
            "macro_rules! println { ($($t:tt)*) => {} } fn f() { println!(\"{}\", \"hi\"); }",
            "macro_rules! println { ($($t:tt)*) => {} } fn f() { println!(\"hi\"); }",
        ),
        (
            "fn f() { panic!(\"{}\", \"a{b\"); }",
            "fn f() { panic!(\"a{{b\"); }",
        ),
        (
            "struct String; fn f() -> String { format!(\"abc\") }",
            "struct String; fn f() -> String { \"abc\".to_string() }",
        ),
    ] {
        separate(written, read);
    }
}
