# syn-canon

[![CI](https://github.com/LucaCappelletti94/cargo-echoes/actions/workflows/ci.yml/badge.svg)](https://github.com/LucaCappelletti94/cargo-echoes/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/syn-canon.svg)](https://crates.io/crates/syn-canon)
[![docs.rs](https://docs.rs/syn-canon/badge.svg)](https://docs.rs/syn-canon)

Canonical forms of [`syn`](https://docs.rs/syn) files. Formatting, supported style folds and local binder names share one comparison key.

```rust
use std::collections::HashSet;

let canonical = |src: &str| syn_canon::canonicalize(syn::parse_str(src).unwrap());

let mut forms = HashSet::new();
assert!(forms.insert(canonical("fn f(x: u32) -> u32 { return (x + 1); }")));
assert!(!forms.insert(canonical("fn g(y: u32) -> u32 { y + 1 }")));
assert!(forms.insert(canonical("fn f(x: u64) -> u64 { x + 1 }")));
assert_eq!(
    canonical("fn f(x: u32) -> bool { x == x }"),
    canonical("fn f(x: u32) -> bool { true }"),
);
assert_eq!(
    canonical("fn f(x: u32) -> u32 { x ^ x }"),
    canonical("fn f(x: u32) -> u32 { 0u32 }"),
);
assert_eq!(
    canonical("fn f(x: u32, y: u32) -> u32 { x.wrapping_add(y) * x }"),
    canonical("fn f(x: u32, y: u32) -> u32 { x * y.wrapping_add(x) }"),
);
assert_eq!(
    canonical("fn f(p: bool, q: bool, x: u32, y: u32) -> u32 { match p { true => if q { x } else { y }, _ => y } }"),
    canonical("fn f(p: bool, q: bool, x: u32, y: u32) -> u32 { if p && q { x } else { y } }"),
);
assert_eq!(
    canonical("fn f(p: bool, x: u32, y: u32) -> u32 { if p { return x; } y }"),
    canonical("fn f(p: bool, x: u32, y: u32) -> u32 { if p { x } else { y } }"),
);
assert_eq!(
    canonical(r#"fn f() -> &'static str { concat!("a", r"\n") }"#),
    canonical(r#"fn f() -> &'static str { "a\\n" }"#),
);
assert_eq!(
    canonical("fn f(p: bool) -> bool { p == false }"),
    canonical("fn f(p: bool) -> bool { !p }"),
);
assert_eq!(
    canonical("fn f(p: bool, q: bool) -> bool { if q { p } else { false } }"),
    canonical("fn f(p: bool, q: bool) -> bool { p && q }"),
);
assert_eq!(
    canonical("fn f(mut x: u32, y: u32) -> u32 { x += y; x }"),
    canonical("fn f(mut x: u32, y: u32) -> u32 { x = x + y; x }"),
);
assert_eq!(
    canonical("fn f() -> u32 { let v: u32 = 5; v }"),
    canonical("fn f() -> u32 { let v = 5u32; v }"),
);
println!("{forms:?}");
```

`no_std` with `alloc`. It powers [`cargo-echoes`](https://github.com/LucaCappelletti94/cargo-echoes), which finds duplicated doctests.

Equal forms mean approximate duplication, since the normalization cannot see what the compiler resolves, such as inferred types, macro expansions or an enclosing `cfg`. Tail `return` folds require temporary-free values and forwarded control heads, preserving return boundaries for unknown calls, borrows, destructor dependencies and unresolved values in both comparison modes. An `if` ending in `return` before a tail reads as an `if` with the tail as its `else` branch only in compiling mode, outside `async` bodies, with a proven condition and a temporary-free tail. Reflexive equality, comparison with a `bool` literal, `&&` and `||` operand order, literals and repeats, `^` cancellation, identity and absorbing operands such as `x + 0`, `x * 1`, `x | 0` and `x & 0`, operand order of `+`, `*`, `wrapping_add`, `wrapping_mul`, `saturating_add`, `saturating_mul` and `abs_diff`, boolean `match` folding, nested `if` merging into `&&` and compound assignment to a local binding require compiling mode and proven total boolean or fixed-width integer operands, and `concat!` folding requires compiling mode, the built-in macro and string literal arguments. In compiling mode a `let` binding's fixed-width integer annotation moves onto an integer literal initializer as its suffix.

Macro input retains significant commas unless lexical resolution proves a supported standard grammar. Enclosing macros and live attributes preserve comma spelling.

## Stability

`CanonicalForm` keys and hashes are comparable within a semver-compatible release series. Before `1.0`, each minor version defines a series, such as `0.5.x`. Incompatible normalization requires a new series.
