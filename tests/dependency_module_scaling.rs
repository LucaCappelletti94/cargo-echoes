//! The cost of a function view against the size of its module.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

/// A module of `count` constants and sibling functions, each function with nested blocks, a
/// loop and a match, reading its constant and calling the function before it.
fn blocks(count: usize) -> syn::File {
    let mut source = String::new();
    for index in 0..count {
        let previous = index.saturating_sub(1);
        let _ = writeln!(
            source,
            "const K{index}: u32 = {index}; pub fn f{index}(a: u32, b: u32) -> u32 {{ let mut acc = K{index}; for v in [a, b, a ^ b] {{ if v > 10 {{ acc = acc.wrapping_add(f{previous}(v, 3)); }} else {{ acc ^= v; }} }} match acc % 3 {{ 0 => acc, 1 => acc + 1, _ => acc + 2 }} }}"
        );
    }
    syn::parse_str(&source).unwrap()
}

/// A module of `count` constants and functions, each function reading 32 of the constants.
fn references(count: usize) -> syn::File {
    let mut source = String::new();
    for index in 0..count {
        let _ = write!(
            source,
            "const K{index}: u32 = {index}; pub fn f{index}(a: u32) -> u32 {{ a"
        );
        for step in 0..32 {
            let _ = write!(source, " ^ K{}", (index + step * 7) % count);
        }
        source.push_str(" }\n");
    }
    syn::parse_str(&source).unwrap()
}

/// The shortest of three timings of every function view of `file`.
fn every_view(file: &syn::File) -> Duration {
    let context =
        syn_canon::SourceContext::new(core::iter::once((&[][..], file)), syn_canon::Edition::E2021);
    (0..3)
        .map(|_| {
            let start = Instant::now();
            for item in &file.items {
                if let syn::Item::Fn(function) = item {
                    let _ = context
                        .function(&function.sig, &function.block)
                        .unwrap()
                        .canonicalize();
                }
            }
            start.elapsed()
        })
        .min()
        .unwrap()
}

#[test]
fn block_analysis_costs_linear_time_in_the_module() {
    let small = every_view(&blocks(25));
    let large = every_view(&blocks(400));
    // Sixteen times the functions take sixteen times as long when one view costs a constant,
    // and about a hundred times as long when its analyzed blocks cost time linear in the module.
    assert!(
        large < small * 40,
        "400 functions took {large:?}, 25 functions took {small:?}"
    );
}

#[test]
fn inherited_references_cost_linear_time_in_the_module() {
    let small = every_view(&references(40));
    let large = every_view(&references(2560));
    // Sixty-four times the functions take sixty-four times as long when one view costs a
    // constant, and about 170 times as long when each reference costs time linear in the module.
    assert!(
        large < small * 110,
        "2560 functions took {large:?}, 40 functions took {small:?}"
    );
}
