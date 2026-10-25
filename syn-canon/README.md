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
println!("{forms:?}");
```

`no_std` with `alloc`. It powers [`cargo-echoes`](https://github.com/LucaCappelletti94/cargo-echoes), which finds duplicated doctests.

Equal forms mean approximate duplication, since the normalization cannot see what the compiler resolves, such as inferred types, macro expansions or an enclosing `cfg`.

Macro input retains significant commas unless lexical resolution proves a supported standard grammar. Enclosing macros and live attributes preserve comma spelling.

## Stability

`CanonicalForm` keys and hashes are comparable within a semver-compatible release series. Before `1.0`, each minor version defines a series, such as `0.5.x`. Incompatible normalization requires a new series.
