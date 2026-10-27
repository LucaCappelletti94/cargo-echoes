//! Public relations for independently executed bound order and lifetime elision witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn bound_order_and_elision_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/bounds_lifetimes_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
