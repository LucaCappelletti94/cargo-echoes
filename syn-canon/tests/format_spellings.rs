//! Newline, empty-string and argument spellings of the standard formatting macros read as one
//! form.

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

fn function(body: &str) -> String {
    format!("fn f(x: u32, y: u32, w: &mut Vec<u8>) {{ {body}; }}")
}

fn rows(rows: &[(&str, &str)]) {
    for (written, read) in rows {
        equivalent(&function(written), &function(read));
    }
}

#[test]
fn trailing_newlines_read_as_line_macros() {
    rows(&[
        ("print!(\"{}\\n\", x)", "println!(\"{}\", x)"),
        ("print!(\"{x} and {y}\\n\")", "println!(\"{x} and {y}\")"),
        ("eprint!(\"done\\n\")", "eprintln!(\"done\")"),
        ("std::print!(\"tab\\t\\n\")", "std::println!(\"tab\\t\")"),
        (
            "let _ = write!(w, \"{:>4}\\n\", x)",
            "let _ = writeln!(w, \"{:>4}\", x)",
        ),
        ("print!(\"{{braces}}\\n\")", "println!(\"{{braces}}\")"),
    ]);
}

#[test]
fn empty_format_strings_read_as_bare_line_macros() {
    rows(&[
        ("println!(\"\")", "println!()"),
        ("eprintln!(\"\")", "eprintln!()"),
        ("let _ = writeln!(w, \"\")", "let _ = writeln!(w)"),
        ("print!(\"\\n\")", "println!()"),
        ("let _ = write!(w, \"\\n\")", "let _ = writeln!(w)"),
    ]);
}

#[test]
fn positional_and_named_identifier_arguments_read_inline() {
    rows(&[
        (
            "let _ = format!(\"{0} {0}\", x)",
            "let _ = format!(\"{x} {x}\")",
        ),
        (
            "let _ = format!(\"{1}-{0}\", x, y)",
            "let _ = format!(\"{y}-{x}\")",
        ),
        (
            "let _ = format!(\"{0:?} {}\", x)",
            "let _ = format!(\"{x:?} {x}\")",
        ),
        (
            "let _ = format!(\"{a}\", a = x)",
            "let _ = format!(\"{x}\")",
        ),
        (
            "let _ = format!(\"{a:>3} {b}\", a = x, b = y)",
            "let _ = format!(\"{x:>3} {y}\")",
        ),
        (
            "let _ = format!(\"{} {a}\", y, a = x)",
            "let _ = format!(\"{y} {x}\")",
        ),
        ("print!(\"{0}\\n\", x)", "println!(\"{x}\")"),
    ]);
}

#[test]
fn raw_carriage_argument_and_local_spellings_stay_apart() {
    for (left, right) in [
        (function("print!(r\"a\\n\")"), function("println!(r\"a\")")),
        (
            function("print!(\"a\\r\\n\")"),
            function("println!(\"a\\r\")"),
        ),
        (
            function("print!(\"a\\n\\n\")"),
            function("println!(\"a\\n\")"),
        ),
        (
            function("print!(\"{}{}\", x, NL)"),
            function("println!(\"{}\", x)"),
        ),
        (
            function("let _ = format!(\"{0} {1}\", x, y + 1)"),
            function("let _ = format!(\"{x} {1}\", y + 1)"),
        ),
        (
            function("let _ = format!(\"{a}\", a = x + 1)"),
            function("let _ = format!(\"{x}\")"),
        ),
        (
            function("let _ = format!(\"{a} {}\", a = x, y)"),
            function("let _ = format!(\"{x} {y}\")"),
        ),
        (
            "macro_rules! println { ($($t:tt)*) => {} }\nfn f(x: u32) { print!(\"{}\\n\", x); }"
                .to_owned(),
            "macro_rules! println { ($($t:tt)*) => {} }\nfn f(x: u32) { println!(\"{}\", x); }"
                .to_owned(),
        ),
    ] {
        separate(&left, &right);
    }
}
