//! In function views, a file-level import of an external crate path reads as the rooted path.

use syn_canon::Edition;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

/// The view of the function `f` in the module at `module` of a context holding `files`.
fn view(files: &[(&[&str], &str)], module: &[&str], edition: Edition) -> syn_canon::CanonicalForm {
    let parsed: Vec<(Vec<String>, syn::File)> = files
        .iter()
        .map(|(path, source)| {
            (
                path.iter().map(|segment| (*segment).to_string()).collect(),
                parse(source),
            )
        })
        .collect();
    let context = syn_canon::SourceContext::new(
        parsed.iter().map(|(path, file)| (path.as_slice(), file)),
        edition,
    );
    let (_, file) = parsed
        .iter()
        .find(|(path, _)| path.iter().eq(module.iter().copied()))
        .expect("the module's file");
    let function = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "f" => Some(function),
            _ => None,
        })
        .expect("a function named f");
    context
        .function(&function.sig, &function.block)
        .expect("an indexed function")
        .canonicalize()
}

fn lone(source: &str, edition: Edition) -> syn_canon::CanonicalForm {
    view(&[(&[], source)], &[], edition)
}

const PAIRS: [(&str, &str); 10] = [
    (
        "use std::mem;\nfn f(a: &mut u32, b: &mut u32) { mem::swap(a, b) }",
        "fn f(a: &mut u32, b: &mut u32) { std::mem::swap(a, b) }",
    ),
    (
        "use std::mem::swap;\nfn f(a: &mut u32, b: &mut u32) { swap(a, b) }",
        "fn f(a: &mut u32, b: &mut u32) { std::mem::swap(a, b) }",
    ),
    (
        "use std::mem::swap as exchange;\nfn f(a: &mut u32, b: &mut u32) { exchange(a, b) }",
        "fn f(a: &mut u32, b: &mut u32) { std::mem::swap(a, b) }",
    ),
    (
        "use std::{fmt, io};\nfn f(w: &mut fmt::Formatter<'_>) -> fmt::Result { w.write_str(\"a\") }\nfn g() -> io::Result<()> { Ok(()) }",
        "fn f(w: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { w.write_str(\"a\") }",
    ),
    (
        "use std::fmt::{self, Display};\nfn f<T: Display>(t: T) -> fmt::Result { Ok(()) }",
        "fn f<T: std::fmt::Display>(t: T) -> std::fmt::Result { Ok(()) }",
    ),
    (
        "use std::io;\nfn f() -> io::Result<()> { Ok(()) }",
        "fn f() -> std::io::Result<()> { Ok(()) }",
    ),
    (
        "use std::collections::HashMap;\nfn f() -> usize { HashMap::<u32, u32>::new().len() }",
        "fn f() -> usize { std::collections::HashMap::<u32, u32>::new().len() }",
    ),
    (
        "use serde::Serialize;\nfn f<T: Serialize>(t: T) -> T { t }",
        "fn f<T: serde::Serialize>(t: T) -> T { t }",
    ),
    (
        "use serde::ser;\nfn f<T: ser::Serialize>(t: T) -> T { t }",
        "fn f<T: serde::ser::Serialize>(t: T) -> T { t }",
    ),
    (
        "use ::serde::Serialize;\nfn f<T: Serialize>(t: T) -> T { t }",
        "fn f<T: ::serde::Serialize>(t: T) -> T { t }",
    ),
];

#[test]
fn imported_external_paths_read_as_rooted_paths() {
    for (imported, rooted) in PAIRS {
        for edition in [Edition::E2018, Edition::E2021, Edition::E2024] {
            assert_eq!(
                lone(imported, edition),
                lone(rooted, edition),
                "{imported}\n{rooted}"
            );
        }
    }
}

#[test]
fn standard_readings_reach_imported_paths() {
    for (imported, method) in [
        (
            "use std::cmp::max;\nfn f(a: u32, b: u32) -> u32 { max(a, b) }",
            "fn f(a: u32, b: u32) -> u32 { a.max(b) }",
        ),
        (
            "use std::cmp;\nfn f(a: u32, b: u32) -> u32 { cmp::min(a, b) }",
            "fn f(a: u32, b: u32) -> u32 { a.min(b) }",
        ),
    ] {
        assert_eq!(
            lone(imported, Edition::E2021),
            lone(method, Edition::E2021),
            "{imported}\n{method}"
        );
    }
}

#[test]
fn nested_declarations_shadow_only_their_namespaces() {
    let pair = |item: &str| {
        (
            lone(
                &format!(
                    "use std::mem;\nfn f(a: &mut u32, b: &mut u32) {{ {item} mem::swap(a, b) }}"
                ),
                Edition::E2021,
            ),
            lone(
                &format!("fn f(a: &mut u32, b: &mut u32) {{ {item} std::mem::swap(a, b) }}"),
                Edition::E2021,
            ),
        )
    };
    for item in [
        "enum mem {}",
        "extern crate mem;",
        "extern crate other as mem;",
        "struct mem;",
        "trait mem {}",
        "trait mem = Sized;",
        "type mem = u8;",
        "union mem { a: u8 }",
        "mod mem {}",
        "use other::mem;",
        "use other::thing as mem;",
        "use other::*;",
    ] {
        let (imported, rooted) = pair(item);
        assert_ne!(imported, rooted, "{item}");
    }
    for item in [
        "const mem: u8 = 0;",
        "static mem: u8 = 0;",
        "fn mem() {}",
        "macro_rules! mem { () => {} }",
        "let mem = 1u8;",
    ] {
        let (imported, rooted) = pair(item);
        assert_eq!(imported, rooted, "{item}");
    }
}

#[test]
fn generic_parameters_keep_naming_the_parameter() {
    for (imported, plain) in [
        (
            "use std::mem;\nfn f<const mem: usize>() -> usize { mem }",
            "fn f<const mem: usize>() -> usize { mem }",
        ),
        (
            "use std::fmt::Debug;\nfn f<Debug: Copy>(value: Debug) -> Debug { value }",
            "fn f<Debug: Copy>(value: Debug) -> Debug { value }",
        ),
    ] {
        assert_eq!(
            lone(imported, Edition::E2021),
            lone(plain, Edition::E2021),
            "{imported}\n{plain}"
        );
    }
}

#[test]
fn qualified_paths_root_their_trait_path() {
    for (imported, rooted) in [
        (
            "use std::fmt;\nfn f<T: fmt::Debug>(t: &T, w: &mut fmt::Formatter<'_>) -> fmt::Result { <T as fmt::Debug>::fmt(t, w) }",
            "fn f<T: std::fmt::Debug>(t: &T, w: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { <T as std::fmt::Debug>::fmt(t, w) }",
        ),
        (
            "use std::iter;\nfn f<I: iter::Iterator>(i: I) -> Option<<I as iter::Iterator>::Item> { None }",
            "fn f<I: std::iter::Iterator>(i: I) -> Option<<I as std::iter::Iterator>::Item> { None }",
        ),
        (
            "use serde::de;\nfn f<S>(s: S) -> u8 { let <S as de::Tr>::Out { a } = s; a }",
            "fn f<S>(s: S) -> u8 { let <S as serde::de::Tr>::Out { a } = s; a }",
        ),
        (
            "use serde::de;\nfn f<S>(s: S) -> u8 { let <S as de::Tr>::Out(a) = s; a }",
            "fn f<S>(s: S) -> u8 { let <S as serde::de::Tr>::Out(a) = s; a }",
        ),
        (
            "use serde::de;\nfn f<S>() -> u8 { <S as de::Tr>::Out { a: 1 }.a }",
            "fn f<S>() -> u8 { <S as serde::de::Tr>::Out { a: 1 }.a }",
        ),
        (
            "use std::mem::swap;\nfn f<T: Tr>() { <T>::swap() }",
            "fn f<T: Tr>() { <T>::swap() }",
        ),
    ] {
        assert_eq!(
            lone(imported, Edition::E2021),
            lone(rooted, Edition::E2021),
            "{imported}\n{rooted}"
        );
    }
}

#[test]
fn imports_in_different_modules_meet() {
    let files: [(&[&str], &str); 3] = [
        (&[], "mod a;\nmod b;"),
        (
            &["a"],
            "use serde::Serialize;\npub fn f<T: Serialize>(t: T) -> T { t }",
        ),
        (
            &["b"],
            "use serde::Serialize;\npub fn f<T: Serialize>(t: T) -> T { t }",
        ),
    ];
    assert_eq!(
        view(&files, &["a"], Edition::E2021),
        view(&files, &["b"], Edition::E2021)
    );
}

#[test]
fn edition_2015_imports_keep_their_spelling() {
    for (imported, rooted) in PAIRS {
        assert_ne!(
            lone(imported, Edition::E2015),
            lone(rooted, Edition::E2015),
            "{imported}\n{rooted}"
        );
    }
}

#[test]
fn local_and_unproven_names_stay_apart() {
    for (imported, rooted) in [
        (
            "mod serde { pub trait Serialize {} }\nuse serde::Serialize;\nfn f<T: Serialize>(t: T) -> T { t }",
            "fn f<T: serde::Serialize>(t: T) -> T { t }",
        ),
        (
            "struct serde;\nuse serde::Serialize;\nfn f<T: Serialize>(t: T) -> T { t }",
            "fn f<T: serde::Serialize>(t: T) -> T { t }",
        ),
        (
            "use core::mem;\nfn f(a: &mut u32, b: &mut u32) { mem::swap(a, b) }",
            "fn f(a: &mut u32, b: &mut u32) { std::mem::swap(a, b) }",
        ),
        (
            "fn f(a: &mut u32, b: &mut u32) { use std::mem::swap; swap(a, b) }",
            "fn f(a: &mut u32, b: &mut u32) { std::mem::swap(a, b) }",
        ),
        (
            "use std::mem::take;\nfn f(a: &mut u32, b: &mut u32) { take(a); }",
            "fn f(a: &mut u32, b: &mut u32) { std::mem::swap(a, b) }",
        ),
        (
            "use std::collections::*;\nfn f() -> usize { HashMap::<u32, u32>::new().len() }",
            "fn f() -> usize { std::collections::HashMap::<u32, u32>::new().len() }",
        ),
        (
            "use std::mem::swap;\nfn f(swap: fn(&mut u32, &mut u32), a: &mut u32, b: &mut u32) { swap(a, b) }",
            "fn f(swap: fn(&mut u32, &mut u32), a: &mut u32, b: &mut u32) { std::mem::swap(a, b) }",
        ),
        (
            "use std::mem;\nfn f(a: &mut u32, b: &mut u32) { mod std { pub mod mem { pub fn swap(_: &mut u32, _: &mut u32) {} } } mem::swap(a, b) }",
            "fn f(a: &mut u32, b: &mut u32) { mod std { pub mod mem { pub fn swap(_: &mut u32, _: &mut u32) {} } } std::mem::swap(a, b) }",
        ),
        (
            "extern crate other as serde;\nuse serde::Serialize;\nfn f<T: Serialize>(t: T) -> T { t }",
            "fn f<T: serde::Serialize>(t: T) -> T { t }",
        ),
        (
            "use std::mem;\n#[observe]\nfn f(a: &mut u32, b: &mut u32) { mem::swap(a, b) }",
            "#[observe]\nfn f(a: &mut u32, b: &mut u32) { std::mem::swap(a, b) }",
        ),
    ] {
        assert_ne!(
            lone(imported, Edition::E2021),
            lone(rooted, Edition::E2021),
            "{imported}\n{rooted}"
        );
    }
}

#[test]
fn whole_file_keys_keep_their_imports() {
    for (imported, rooted) in PAIRS {
        for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
            assert_ne!(
                canonicalize(parse(imported)),
                canonicalize(parse(rooted)),
                "{imported}\n{rooted}"
            );
        }
    }
}

#[test]
fn editions_parse_from_manifest_spellings() {
    for (text, edition) in [
        ("2015", Edition::E2015),
        ("2018", Edition::E2018),
        ("2021", Edition::E2021),
        ("2024", Edition::E2024),
    ] {
        assert_eq!(text.parse::<Edition>(), Ok(edition));
    }
    assert_eq!(
        "2019".parse::<Edition>().unwrap_err().to_string(),
        "unknown Rust edition"
    );
    assert!(!Edition::E2015.uses_extern_prelude());
    assert!(Edition::E2018.uses_extern_prelude());
}
