//! Significant punctuation across comparison surfaces.

#[path = "support/view.rs"]
mod view;

use syn_canon::ContextKind;
use view::view_of;

const STANDARD_MACROS: [(&str, &str); 21] = [
    ("assert", "1 == 1"),
    ("assert_eq", "1, 1"),
    ("assert_ne", "1, 2"),
    ("dbg", "1"),
    ("debug_assert", "1 == 1"),
    ("debug_assert_eq", "1, 1"),
    ("debug_assert_ne", "1, 2"),
    ("eprint", "\"a\""),
    ("eprintln", "\"a\""),
    ("format", "\"a\""),
    ("format_args", "\"a\""),
    ("matches", "1, 1"),
    ("panic", "\"a\""),
    ("print", "\"a\""),
    ("println", "\"a\""),
    ("todo", "\"a\""),
    ("unimplemented", "\"a\""),
    ("unreachable", "\"a\""),
    ("vec", "1, 2"),
    ("write", "std::io::stdout(), \"a\""),
    ("writeln", "std::io::stdout(), \"a\""),
];

fn file(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn compare(left: &str, right: &str, equal: bool) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        let a = canonicalize(file(left));
        let b = canonicalize(file(right));
        assert_eq!(a == b, equal, "{left}\n{right}\n{a:?}\n{b:?}");
    }
}

fn bodies_of(source: &str) -> Vec<String> {
    let file = file(source);
    let mut forms = Vec::new();
    syn_canon::contexts(&file, &mut |context, form| {
        if context.kind == ContextKind::FunctionBody {
            forms.push(form.to_string());
        }
    });
    forms
}

#[test]
fn proven_std_macros_fold_their_top_level_trailing_comma() {
    for (name, args) in STANDARD_MACROS {
        for root in ["", "std::"] {
            compare(
                &format!("use std::io::Write as _; fn f() {{ {root}{name}!({args}); }}"),
                &format!("use std::io::Write as _; fn f() {{ {root}{name}!({args},); }}"),
                true,
            );
        }
    }
}

#[test]
fn proven_core_and_alloc_macros_fold_their_top_level_trailing_comma() {
    for (name, args) in [
        ("core::assert", "true"),
        ("core::matches", "1, 1"),
        ("alloc::vec", "1, 2"),
    ] {
        compare(
            &format!("extern crate alloc; fn f() {{ {name}!({args}); }}"),
            &format!("extern crate alloc; fn f() {{ {name}!({args},); }}"),
            true,
        );
    }
}

#[test]
fn unsupported_standard_root_leaves_keep_their_punctuation() {
    for name in ["core::dbg", "core::vec", "alloc::println"] {
        compare(
            &format!("fn f() {{ {name}!(1); }}"),
            &format!("fn f() {{ {name}!(1,); }}"),
            false,
        );
    }
}

#[test]
fn proven_std_aliases_fold_their_trailing_comma() {
    for (imports, name) in [
        ("use std::vec as v;", "v"),
        ("use std as library;", "library::vec"),
        ("use core::assert as check;", "check"),
        ("extern crate std as library;", "library::vec"),
        ("extern crate alloc as library;", "library::vec"),
        ("use std::{self as library};", "library::vec"),
    ] {
        compare(
            &format!("{imports} fn f() {{ {name}!(true); }}"),
            &format!("{imports} fn f() {{ {name}!(true,); }}"),
            true,
        );
    }
}

#[test]
fn custom_standard_names_keep_their_trailing_comma_significant() {
    for (name, args) in STANDARD_MACROS {
        for qualified in [false, true] {
            let definition = format!(
                "macro_rules! {name} {{ ({args}) => {{ 10u32 }}; ({args},) => {{ 20u32 }}; }}"
            );
            let (prefix, path) = if qualified {
                (
                    format!("mod custom {{ {definition} pub(crate) use {name}; }}"),
                    format!("custom::{name}"),
                )
            } else {
                (definition, name.to_owned())
            };
            compare(
                &format!("{prefix} fn f() -> u32 {{ {path}!({args}) }}"),
                &format!("{prefix} fn f() -> u32 {{ {path}!({args},) }}"),
                false,
            );
        }
    }
}

#[test]
fn unproven_aliases_keep_their_trailing_comma_significant() {
    for imports in [
        "mod helper { macro_rules! vec { ($($t:tt)*) => {} } pub(crate) use vec; } use helper::vec as local;",
        "mod other; use other::vec as local;",
    ] {
        compare(
            &format!("{imports} fn f() {{ local!(1, 2); }}"),
            &format!("{imports} fn f() {{ local!(1, 2,); }}"),
            false,
        );
    }
}

#[test]
fn glob_imports_keep_their_macro_trailing_commas_significant() {
    for imports in [
        "mod helper { macro_rules! vec { ($($t:tt)*) => {} } pub(crate) use vec; } use helper::*;",
        "use std::*;",
    ] {
        compare(
            &format!("{imports} fn f() {{ vec!(1, 2); }}"),
            &format!("{imports} fn f() {{ vec!(1, 2,); }}"),
            false,
        );
    }
}

#[test]
fn unproven_outer_macros_keep_nested_macro_punctuation_significant() {
    for inner in ["vec", "std::vec"] {
        let prefix = "macro_rules! observe { ($($t:tt)*) => { stringify!($($t)*) } }";
        compare(
            &format!("{prefix} fn f() -> &'static str {{ observe!({inner}!(1)) }}"),
            &format!("{prefix} fn f() -> &'static str {{ observe!({inner}!(1,)) }}"),
            false,
        );
    }
}

#[test]
fn proven_outer_macros_allow_nested_standard_equivalences() {
    compare(
        "fn f() { vec![std::vec![1,]]; }",
        "fn f() { vec![std::vec![1]]; }",
        true,
    );
    compare(
        "fn f() { matches!(std::vec![1,].len(), length if length == 1,); }",
        "fn f() { matches!(std::vec![1].len(), length if length == 1); }",
        true,
    );
}

#[test]
fn one_tuple_commas_inside_macro_arguments_stay_significant() {
    compare(
        "fn f() { let _ = vec![(1,)]; }",
        "fn f() { let _ = vec![(1)]; }",
        false,
    );
    let prefix = "macro_rules! m { ($($t:tt)*) => {} }";
    compare(
        &format!("{prefix} fn f() {{ m!((1,)); }}"),
        &format!("{prefix} fn f() {{ m!((1)); }}"),
        false,
    );
}

#[test]
fn pattern_macro_commas_stay_significant() {
    let prefix = "macro_rules! m { ((a, b)) => { (1, _) }; ((a, b),) => { (_, 2) }; }";
    compare(
        &format!("{prefix} fn f() -> u32 {{ match (1u32, 3u32) {{ m!((a, b)) => 1, _ => 0 }} }}"),
        &format!("{prefix} fn f() -> u32 {{ match (1u32, 3u32) {{ m!((a, b),) => 1, _ => 0 }} }}"),
        false,
    );
}

#[test]
fn macro_definition_patterns_keep_their_punctuation() {
    compare(
        "macro_rules! m { ((1,)) => { 10 }; } fn f() { m!((1,)); }",
        "macro_rules! m { ((1)) => { 10 }; } fn f() { m!((1,)); }",
        false,
    );
}

#[test]
fn invalid_repetition_trailing_commas_stay_distinct() {
    assert_ne!(
        syn_canon::canonicalize_failing(file("fn f() { let _ = vec![1; 2,]; }")),
        syn_canon::canonicalize_failing(file("fn f() { let _ = vec![1; 2]; }")),
    );
}

#[test]
fn observer_attributes_keep_their_item_punctuation_opaque() {
    compare(
        "#[observe] fn f() { vec![1, 2,]; }",
        "#[observe] fn f() { vec![1, 2]; }",
        false,
    );
}

#[test]
fn lexical_contexts_keep_the_macro_punctuation_contracts() {
    for (prefix, path, equal) in [
        ("", "vec", true),
        ("use std::vec as v;", "v", true),
        ("macro_rules! vec { ($($t:tt)*) => {} }", "vec", false),
        (
            "mod custom { macro_rules! vec { ($($t:tt)*) => {} } pub(crate) use vec; }",
            "custom::vec",
            false,
        ),
    ] {
        let bodies = bodies_of(&format!(
            "{prefix} fn first() {{ {path}![1, 2,]; }} fn second() {{ {path}![1, 2]; }}"
        ));
        assert_eq!(bodies.len(), 2);
        assert_eq!(bodies[0] == bodies[1], equal, "{prefix}\n{bodies:?}");
    }
}

#[test]
fn function_views_keep_the_macro_punctuation_contracts() {
    for (prefix, path, equal) in [
        ("", "vec", true),
        ("use std::vec as v;", "v", true),
        ("use std::*;", "vec", false),
        ("macro_rules! vec { ($($t:tt)*) => {} }", "vec", false),
        (
            "mod custom { macro_rules! vec { ($($t:tt)*) => {} } pub(crate) use vec; }",
            "custom::vec",
            false,
        ),
    ] {
        let file = file(&format!(
            "{prefix} fn first() {{ {path}![1, 2,]; }} fn second() {{ {path}![1, 2]; }}"
        ));
        let a = view_of(&file, "first");
        let b = view_of(&file, "second");
        assert_eq!(a == b, equal, "{prefix}\n{a:?}\n{b:?}");
    }
}

#[test]
fn ambiguous_and_gated_imports_keep_macro_punctuation() {
    for imports in [
        "mod custom { macro_rules! vec { ($($t:tt)*) => {} } pub(crate) use vec; } use custom::vec as v; use std::vec as v;",
        "#[cfg(any())] use std::vec as v;",
    ] {
        compare(
            &format!("{imports} fn first() {{ v![1, 2]; }}"),
            &format!("{imports} fn first() {{ v![1, 2,]; }}"),
            false,
        );
        let source = format!("{imports} fn first() {{ v![1, 2]; }} fn second() {{ v![1, 2,]; }}");
        let file = file(&source);
        assert_ne!(view_of(&file, "first"), view_of(&file, "second"));
        let bodies = bodies_of(&source);
        assert_ne!(bodies[0], bodies[1]);
    }
}

#[test]
fn scoped_standard_imports_respect_textual_macros() {
    let rule = "macro_rules! vec { (1) => { 10u32 }; (1,) => { 20u32 }; }";
    for (prefix, body) in [
        (format!("{rule} use std::vec;"), ""),
        (format!("use std::vec; {rule}"), ""),
        (rule.to_owned(), "use std::vec;"),
    ] {
        compare(
            &format!("{prefix} fn f() -> u32 {{ {body} vec!(1) }}"),
            &format!("{prefix} fn f() -> u32 {{ {body} vec!(1,) }}"),
            false,
        );
    }
}

#[test]
fn scoped_attributes_preserve_macro_punctuation() {
    let functions = "fn first() { vec![1, 2]; } fn second() { vec![1, 2,]; }";
    let observed = "#[observe] fn first() { vec![1, 2]; } #[observe] fn second() { vec![1, 2,]; }";
    for source in [
        format!("#![observe] {functions}"),
        observed.to_owned(),
        format!("#[observe] mod nested {{ {functions} }}"),
        format!("mod nested {{ {observed} }}"),
        format!("struct S; #[observe] impl S {{ {functions} }}"),
        format!("struct S; impl S {{ {observed} }}"),
        format!("trait S {{ {observed} }}"),
    ] {
        let bodies = bodies_of(&source);
        assert_eq!(bodies.len(), 2, "{source}");
        assert_ne!(bodies[0], bodies[1], "{source}");
    }
    let bodies = bodies_of(
        "#[observe] fn first() { vec![1, 2,]; } fn second() { vec![1, 2]; } fn third() { vec![1, 2,]; }",
    );
    assert_ne!(bodies[0], bodies[1]);
    assert_eq!(bodies[1], bodies[2]);
}

#[test]
fn raw_shadowed_roots_stay_unproven_in_standard_arguments() {
    compare(
        "mod custom; use custom as r#std; fn f() { vec![r#std::vec!(1)]; }",
        "mod custom; use custom as r#std; fn f() { vec![r#std::vec!(1,)]; }",
        false,
    );
}

#[test]
fn raw_extern_aliases_resolve_inside_standard_arguments() {
    compare(
        "fn f() { vec![{ extern crate std as r#library; r#library::vec![1] }]; }",
        "fn f() { vec![{ extern crate std as r#library; r#library::vec![1,] }]; }",
        true,
    );
}
