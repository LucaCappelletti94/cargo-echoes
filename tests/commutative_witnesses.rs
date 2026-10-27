//! Public relations for independently executed commutative operator and method witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn commutative_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("fixtures/dependency/commutative_cases.json")).unwrap();
    relations(&cases);
}
