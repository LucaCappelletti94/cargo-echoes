//! Public relations for independently executed native `wrapping_add` witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn native_wrapping_add_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("fixtures/dependency/wrapping_add_cases.json")).unwrap();
    relations(&cases);
}
