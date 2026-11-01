//! Unsuffixed literal tails and returns typed by the declared return type.

#[path = "support/canon.rs"]
mod canon;
#[path = "support/parse.rs"]
mod parse;
use canon::{equivalent, separate};

#[test]
fn tails_and_returns_take_the_declared_type() {
    for (written, typed) in [
        ("fn f() -> u32 { 5 }", "fn f() -> u32 { 5u32 }"),
        ("fn f() -> u32 { 2 + 3 }", "fn f() -> u32 { 5u32 }"),
        ("fn f() -> i64 { { 3 } }", "fn f() -> i64 { { 3i64 } }"),
        (
            "fn f(p: bool) -> u8 { if p { return 7; } 9 }",
            "fn f(p: bool) -> u8 { if p { return 7u8; } 9u8 }",
        ),
        (
            "fn f(p: bool) -> u8 { if p { 7 } else { 9 } }",
            "fn f(p: bool) -> u8 { if p { 7u8 } else { 9u8 } }",
        ),
        (
            "fn f(p: bool) -> u32 { if p { 1 } else { 0 } }",
            "fn f(p: bool) -> u32 { p as u32 }",
        ),
        (
            "fn f(p: bool) -> i16 { loop { if p { return -1; } } }",
            "fn f(p: bool) -> i16 { loop { if p { return -1i16; } } }",
        ),
        ("async fn f() -> u32 { 5 }", "async fn f() -> u32 { 5u32 }"),
        (
            "struct S; impl S { fn f(&self) -> u64 { 0 } }",
            "struct S; impl S { fn f(&self) -> u64 { 0u64 } }",
        ),
    ] {
        equivalent(written, typed);
    }
}

#[test]
fn generic_opaque_shadowed_and_closure_returns_stay_unsuffixed() {
    for (written, typed) in [
        ("fn f<u32>() -> u32 { 5 }", "fn f<u32>() -> u32 { 5u32 }"),
        ("fn f() -> impl Copy { 5 }", "fn f() -> impl Copy { 5u32 }"),
        ("fn f() { let _ = 5; }", "fn f() { let _ = 5u32; }"),
        (
            "type u32 = u8; fn f() -> u32 { 5 }",
            "type u32 = u8; fn f() -> u32 { 5u32 }",
        ),
        (
            "fn f() -> u32 { let g = || { return 5; }; g() }",
            "fn f() -> u32 { let g = || { return 5u32; }; g() }",
        ),
        (
            "fn f() -> u32 { let g = async { return 5; }; 0 }",
            "fn f() -> u32 { let g = async { return 5u32; }; 0 }",
        ),
        ("fn f() -> usize { 5 }", "fn f() -> usize { 5usize }"),
        (
            "fn f() -> u32 { fn g() -> u8 { 1 } g() as u32 }",
            "fn f() -> u32 { fn g() -> u8 { 1u32 } g() as u32 }",
        ),
    ] {
        separate(written, typed);
    }
}
