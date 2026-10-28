//! Public relations for independently executed bound order and impl lifetime witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn bounds_impl_lifetimes_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/bounds_impl_lifetimes_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
