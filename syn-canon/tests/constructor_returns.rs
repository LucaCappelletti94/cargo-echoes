//! Tail returns of prelude constructors read as tails.

#[path = "support/canon.rs"]
mod canon;
#[path = "support/parse.rs"]
mod parse;
use canon::{equivalent, separate};
use parse::parse;

#[path = "support/view.rs"]
mod view;

#[test]
fn constructor_returns_match_their_tails() {
    for (signature, returned, tail) in [
        ("-> Result<(), u8>", "return Ok(());", "Ok(())"),
        ("-> Result<u32, u8>", "return Err(1u8);", "Err(1u8)"),
        ("-> Option<u32>", "return Some(x);", "Some(x)"),
        ("-> Option<u32>", "return None;", "None"),
        (
            "-> Option<(u32, bool)>",
            "return Some((x, true));",
            "Some((x, true))",
        ),
        (
            "-> Result<Option<u32>, u8>",
            "return Ok(Some(x));",
            "Ok(Some(x))",
        ),
    ] {
        equivalent(
            &format!("fn f(x: u32) {signature} {{ mark(x); {returned} }}"),
            &format!("fn f(x: u32) {signature} {{ mark(x); {tail} }}"),
        );
    }
}

#[test]
fn early_constructor_returns_match_their_branches() {
    equivalent(
        "fn f(p: bool, x: u32) -> Result<u32, u8> { if p { return Err(1u8); } Ok(x) }",
        "fn f(p: bool, x: u32) -> Result<u32, u8> { if p { Err(1u8) } else { Ok(x) } }",
    );
    equivalent(
        "fn f(p: bool, x: u32) -> Option<u32> { if p { return None; } Some(x) }",
        "fn f(p: bool, x: u32) -> Option<u32> { if p { None } else { Some(x) } }",
    );
    equivalent(
        "fn f() { let _ = |p: bool| -> Option<u32> { if p { return None; } Some(1u32) }; }",
        "fn f() { let _ = |p: bool| -> Option<u32> { if p { None } else { Some(1u32) } }; }",
    );
}

#[test]
fn other_returns_keep_their_spelling() {
    for (signature, returned, tail) in [
        (
            "-> Result<usize, u8>",
            "return Ok(make().len());",
            "Ok(make().len())",
        ),
        ("-> Option<String>", "return Some(make());", "Some(make())"),
        (
            "-> Option<u32>",
            "return Some(x).map(add);",
            "Some(x).map(add)",
        ),
    ] {
        separate(
            &format!("fn f(x: u32) {signature} {{ mark(x); {returned} }}"),
            &format!("fn f(x: u32) {signature} {{ mark(x); {tail} }}"),
        );
    }
    for prefix in [
        "use custom::Some;",
        "fn Some(x: u32) -> Option<u32> { None }",
        "use other::*;",
        "#![no_implicit_prelude]",
    ] {
        separate(
            &format!("{prefix} fn f(x: u32) -> Option<u32> {{ return Some(x); }}"),
            &format!("{prefix} fn f(x: u32) -> Option<u32> {{ Some(x) }}"),
        );
    }
    separate(
        "#[observe] fn f(x: u32) -> Option<u32> { return Some(x); }",
        "#[observe] fn f(x: u32) -> Option<u32> { Some(x) }",
    );
}

#[test]
fn function_views_read_constructor_returns() {
    let file = parse(
        "fn first(x: u32) -> Option<u32> { return Some(x); } fn second(y: u32) -> Option<u32> { Some(y) }",
    );
    assert_eq!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
    let file = parse(
        "use custom::Ok; fn first() -> Result<(), u8> { return Ok(()); } fn second() -> Result<(), u8> { Ok(()) }",
    );
    assert_ne!(
        view::view_of(&file, "first"),
        view::view_of(&file, "second")
    );
}
