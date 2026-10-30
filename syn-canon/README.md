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
    canonical("fn f(x: u32, y: u32) -> u32 { match x { 0 => y, 1 => y, _ => x } }"),
    canonical("fn f(x: u32, y: u32) -> u32 { match x { 0 | 1 => y, _ => x } }"),
);
assert_eq!(
    canonical("fn f(o: Option<u32>) -> u32 { let Some(v) = o else { return 0; }; v }"),
    canonical("fn f(o: Option<u32>) -> u32 { match o { Some(v) => { v } _ => { return 0; } } }"),
);
assert_eq!(
    canonical(r#"fn f() -> String { "a".to_owned() }"#),
    canonical(r#"fn f() -> String { String::from("a") }"#),
);
assert_eq!(
    canonical("fn f() -> Vec<u32> { vec![] }"),
    canonical("fn f() -> Vec<u32> { Vec::new() }"),
);
assert_eq!(
    canonical("fn f(x: u32) -> u64 { x as u64 }"),
    canonical("fn f(x: u32) -> u64 { u64::from(x) }"),
);
assert_eq!(
    canonical("fn f<'a, T: Copy + Clone>(x: &'a T) -> &'a T { x }"),
    canonical("fn f<T>(x: &T) -> &T where T: Clone + Copy { x }"),
);
assert_eq!(
    canonical("fn f(x: u32, y: u32) -> bool { &x.clone() == &y }"),
    canonical("fn f(x: u32, y: u32) -> bool { x == y }"),
);
assert_eq!(
    canonical("fn f(p: bool) -> Result<(), u8> { if p { return Err(1); } return Ok(()); }"),
    canonical("fn f(p: bool) -> Result<(), u8> { if p { Err(1) } else { Ok(()) } }"),
);
assert_eq!(
    canonical("fn f(p: bool) { if p { return; } a(); b(); }"),
    canonical("fn f(p: bool) { if !p { a(); b(); } }"),
);
assert_eq!(
    canonical("fn f(o: Option<u32>) -> u32 { if let Some(v) = o { v } else { 0 } }"),
    canonical("fn f(o: Option<u32>) -> u32 { match o { None => 0, Some(v) => v } }"),
);
assert_eq!(
    canonical("fn f(v: &mut Vec<u32>) { let _ = v.pop(); }"),
    canonical("fn f(v: &mut Vec<u32>) { v.pop(); }"),
);
assert_eq!(
    canonical("fn f(x: u32) -> bool { x >= 1 && x <= 9 }"),
    canonical("fn f(x: u32) -> bool { x != 0 && x < 10 }"),
);
assert_eq!(
    canonical("fn f(x: u32, y: u32) -> u32 { if x >= y { x } else { y } }"),
    canonical("fn f(x: u32, y: u32) -> u32 { std::cmp::max(y, x) }"),
);
assert_eq!(
    canonical("fn f(v: &[u32]) -> bool { for x in v.iter() { g(x); } v.len() == 0 }"),
    canonical("fn f(v: &[u32]) -> bool { for x in v { g(x); } v.is_empty() }"),
);
assert_eq!(
    canonical(r#"fn f(n: u32) -> String { println!("{} {}", "n", n); format!("") }"#),
    canonical(r#"fn f(n: u32) -> String { println!("n {n}"); String::new() }"#),
);
assert_eq!(
    canonical("struct S<'a>(&'a u8); impl<'a> S<'a> { fn f(&self) -> Box<dyn Sync + Send> { loop {} } }"),
    canonical("struct S<'a>(&'a u8); impl S<'_> { fn f(&self) -> Box<dyn Send + Sync> { loop {} } }"),
);
assert_eq!(
    canonical("fn f(x: u32, y: i32) -> (u32, bool, u8) { (x / 8, y % 2 == 0, u8::MAX) }"),
    canonical("fn f(x: u32, y: i32) -> (u32, bool, u8) { (x >> 3, y & 1 == 0, 255u8) }"),
);
assert_eq!(
    canonical("fn f(x: u32) -> bool { matches!(x, 3 | 1) || (5..=9).contains(&x) }"),
    canonical("fn f(x: u32) -> bool { x == 1 || x == 3 || x >= 5 && x <= 9 }"),
);
assert_eq!(
    canonical("fn f(p: bool) -> u32 { if p { 1 } else { 0 } }"),
    canonical("fn f(p: bool) -> u32 { p as u32 }"),
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

Equal forms mean approximate duplication, since the normalization cannot see what the compiler resolves, such as inferred types, macro expansions or an enclosing `cfg`. Tail `return` folds require temporary-free values and forwarded control heads, preserving return boundaries for unknown calls, borrows, destructor dependencies and unresolved values in both comparison modes. In compiling mode, `Ok`, `Err` and `Some` of such a value and `None` count as values when the standard prelude is active and no binding claims the name. An `if` ending in `return` before a tail reads as an `if` with the tail as its `else` branch only in compiling mode, outside `async` bodies, with a proven condition and a temporary-free tail. In compiling mode, outside `async` bodies, an `if` ending in `return;` in a unit body or in a `continue` of its loop reads as an `if` with the following statements as its `else` branch, or as `if !p` when nothing precedes the jump, when those statements end in a statement and declare no item, and a `loop` that opens with `if p { break; }` reads as `while !p`. In compiling mode, adjacent `match` arms with the same body join into an or-pattern when no pattern names an identifier other than the prelude `None`, and a `let`-`else` reads as `match` when it matches a local or parameter in a body without items and the rest of the block ends in no temporary. In compiling mode an `if let` without a `let` chain over a local, parameter or closure input reads as a `match` whose `_` arm holds the `else` branch or the unit block, and adjacent unguarded arms of distinct literals, of distinct prelude `Option` or `Result` variants or of distinct variants `E::V` of an enum declared in view take one order up to the first arm that matches every value. Under the standard prelude a two-arm `match` of `None` and `Some(p)` with an irrefutable `p` reads as `Some(p)` followed by `_`. Under the standard prelude, `"…".to_owned()` and `String::from("…")` read as `"…".to_string()`, the empty literal's spellings and `format!("")` as `String::new()`, a standard `format!` of a literal without placeholders as its text's `to_string()`, and `vec![]` as `Vec::new()` in compiling mode, unless the crate root carries `no_std`, a module in the chain carries `no_implicit_prelude`, a glob or binding claims `String`, `ToString`, `ToOwned` or `Vec`, or a trait in view declares `to_string` or `to_owned`. In compiling mode a string literal argument of a plain `{}` placeholder of a standard formatting macro other than `panic!` moves into the format string with its braces doubled. In compiling mode the `where` predicates of one bounded type merge, their bounds sort and so do the bounds of `dyn` and `impl Trait` types, supertraits and associated types, unless a bound is of the `Fn` family or binds an associated item. A function lifetime parameter the elision rules assign exactly, named by one input reference or the reference receiver, by no bound and by no body, gives way to elision, and an impl lifetime parameter named once in the header as a path argument or a reference lifetime, by no bound and by no item, reads as `'_`. Reflexive equality, comparison with a `bool` literal, `&&` and `||` operand order, literals and repeats, `^` cancellation, identity and absorbing operands such as `x + 0`, `x * 1`, `x | 0`, `x & 0` and `x % 1`, operand order of `+`, `*`, `wrapping_add`, `wrapping_mul`, `saturating_add`, `saturating_mul` and `abs_diff`, boolean `match` folding, nested `if` merging into `&&`, compound assignment to a local binding, a cast to the operand's own type, a lossless widening `T::from(x)` read as `x as T`, `*&x`, `&x == &y`, `!!x`, `x.pow(1)` and `x.clone()` read as their copies, an unsigned `x / 2^k` and `x % 2^k` read as `x >> k` and `x & (2^k - 1)` and `x % 2` compared with zero read as `x & 1` require compiling mode and proven total boolean or fixed-width integer operands, the widening also the prelude's `From`, and `clone` also the prelude, standard-library imports alone and no trait in view declaring `clone`. The same proof reads an inclusive comparison of an integer with an integer literal as the strict comparison with the adjacent literal when that literal stays in the type, and over unsigned operands `0 < x` as `0 != x` and `x < 1` as `0 == x`. The same proof reads `if p { 1 } else { 0 }` over literals of one proven type `T` as `p as T` and the swapped literals as `!p as T`, and `max` and `min` of two integers in method, `std::cmp` and conditional form as one method call with sorted operands, under the prelude with `Ord` unclaimed. `concat!` folding requires compiling mode, the built-in macro and string literal arguments. In compiling mode a `let` binding's fixed-width integer annotation moves onto an integer literal initializer as its suffix and into the branch tails of a conditional initializer, and `let _ = e;` reads as `e;` when `e` is a call, method call or `.await` and the `let` carries no attribute or annotation. In compiling mode a byte literal, a cast of an unsuffixed integer literal or of an ASCII character or byte to a proven integer type, and `MAX` and `MIN` of a proven fixed-width type read as the suffixed literal, a pointer-sized target taking only values every pointer width holds. A binding declared as `Vec<T>`, `[T; N]`, `[T]`, `String` or `str` behind at most one reference reads `for x in v.iter()` as `for x in &v`, or `for x in v` for a shared reference, the `iter_mut` and owned `Vec` `into_iter` loops alike, and `v.len() == 0` as `v.is_empty()`, in compiling mode under the prelude with standard-library imports alone and no trait in view declaring `iter`, `iter_mut`, `into_iter`, `len`, `is_empty` or `contains`. In compiling mode a standard `matches!` without a guard over a binding of a proven fixed-width integer type, with disjoint integer literal and literal range patterns of that type, reads as its chain of comparisons, so `matches!(x, 1 | 3..=5)` meets `x == 1 || x >= 3 && x <= 5`, and `(a..=b).contains(&x)` and the other literal ranges' `contains` read alike under the collection condition. A function's declared return type, when it names a fixed-width integer type, types its body's tail and its `return` values the same way, outside closures and `async` blocks.

Macro input retains significant commas unless lexical resolution proves a supported standard grammar. Enclosing macros and live attributes preserve comma spelling.

A function view from `SourceContext` under Rust 2018 or later reads a file-level import of an external crate item as the item's rooted path wherever the import is in view, so `use std::mem;` then `mem::swap` matches `std::mem::swap`. The module must declare and import no other item of the crate's name. Whole-file keys, imports inside function bodies, observed functions and Rust 2015 keep the written spelling.

In compiling mode, a `match`, `if let`, `while let` or `let` over a place whose patterns bind only by `ref`, or only by `ref mut`, reads as the same patterns without `ref` over a borrow of the place, so `match o { Some(ref s) => … }` matches `match &o { Some(s) => … }`. A parameter declared as a shared reference reads `*r` as `r` there. A by-value binding, a reference pattern or a plain name that resolution cannot prove a constant keeps the written form.

In compiling mode, `let y = e; y` and `let y = e; return y;` at the end of a tail block read as `e` when `y` is a plain binding without annotation and `e` passes the temporary-free value proof of tail returns, a built-in `concat!` of string literals included.

In compiling mode, a standard `matches!(e, P)` reads as `match e { P => true, _ => false }`, with a guard kept, and `!match e { … }` whose every arm yields a `bool` literal reads with each literal negated. A body that declares its own `matches!` macro keeps the call.

In compiling mode, with the standard prelude names unclaimed, a binding declared as `Option<T>` or `Result<T, E>`, owned or behind one reference, reads `is_some`, `is_none`, `is_ok` and `is_err` as the `match` they perform, and `o == None` or `o != None` as `is_none` or `is_some`. A written `match` whose first arm is `None` or `Err(_)` and whose second is `_` reads with `Some(_)` or `Ok(_)` leading, so `matches!(o, None)` meets `o.is_none()`. On an owned `Option`, `unwrap_or(c)` and `unwrap_or_else(|| c)` with a literal or primitive binding `c` read as their `match`, and `match o { Some(v) => Some(v), None => None }` reads as `o`. `std::mem::replace(o, None)` over a `&mut Option` reads as `o.take()`, and `Option::Some`, `Option::None`, `Result::Ok` and `Result::Err` read as their bare names.

In compiling mode, a binding declared as `&str` reads `to_owned()` and `String::from(s)` as `s.to_string()`, and a binding declared as `String` or `&String` reads `to_owned()`, `clone()` and `String::from(s.as_str())` the same way. Each copy allocates the length of the text. A file that declares a trait method named `to_string`, `to_owned`, `clone` or `as_str`, imports from outside the standard library, or claims `String`, `ToString`, `ToOwned` or `Clone` keeps the written form, and `format!("{s}")` stays apart because its buffer grows past the text.

## Stability

`CanonicalForm` keys and hashes are comparable within a semver-compatible release series. Before `1.0`, each minor version defines a series, such as `0.5.x`. Incompatible normalization requires a new series.
