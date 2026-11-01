//! A tail or return that reads a binding initialized just before it reads as the initializer
//! when the initializer is a temporary-free value.

#[path = "support/canon.rs"]
mod canon;
#[path = "support/parse.rs"]
mod parse;
use canon::{equivalent, separate};

#[test]
fn bound_values_read_as_their_initializer() {
    for (bound, direct) in [
        (
            "fn f(x: u32) -> u32 { let y = x + 1; y }",
            "fn f(x: u32) -> u32 { x + 1 }",
        ),
        (
            "fn f(x: u32) -> u32 { let y = x + 1; return y; }",
            "fn f(x: u32) -> u32 { x + 1 }",
        ),
        (
            "fn f(x: u32) -> u32 { if x > 1 { let y = x * 2; y } else { 0 } }",
            "fn f(x: u32) -> u32 { if x > 1 { x * 2 } else { 0 } }",
        ),
        (
            "fn f(x: u32) -> u32 { match x { 0 => 1, _ => { let y = x - 1; y } } }",
            "fn f(x: u32) -> u32 { match x { 0 => 1, _ => { x - 1 } } }",
        ),
        (
            "fn f(x: u32) -> bool { let ok = x > 3 && x < 9; ok }",
            "fn f(x: u32) -> bool { x > 3 && x < 9 }",
        ),
        (
            "fn f(p: bool, q: bool) -> bool { if p { let v = q; v } else { false } }",
            "fn f(p: bool, q: bool) -> bool { p && q }",
        ),
        (
            "fn f(x: u32) -> Option<u32> { let found = Some(x); found }",
            "fn f(x: u32) -> Option<u32> { Some(x) }",
        ),
        (
            "fn f(x: u32) -> (u32, bool) { let pair = (x, x > 2); pair }",
            "fn f(x: u32) -> (u32, bool) { (x, x > 2) }",
        ),
    ] {
        equivalent(bound, direct);
    }
}

#[test]
fn temporaries_annotations_and_shared_bindings_stay_apart() {
    for (left, right) in [
        (
            "struct D; impl D { fn val(&self) -> u32 { 1 } }\nfn f() -> u32 { let _a = D; let v = D.val(); v }",
            "struct D; impl D { fn val(&self) -> u32 { 1 } }\nfn f() -> u32 { let _a = D; D.val() }",
        ),
        (
            "fn f(x: u32) -> u32 { let y: u32 = x + 1; y }",
            "fn f(x: u32) -> u32 { x + 1 }",
        ),
        (
            "fn f(x: u32) -> u32 { let mut y = x + 1; y += 1; y }",
            "fn f(x: u32) -> u32 { x + 1 + 1 }",
        ),
        (
            "fn f(x: u32) -> u32 { let y = x + 1; y + y }",
            "fn f(x: u32) -> u32 { (x + 1) + (x + 1) }",
        ),
        (
            "fn f(x: u32) -> u32 { let y = g(x); y }",
            "fn f(x: u32) -> u32 { g(x) }",
        ),
    ] {
        separate(left, right);
    }
}
