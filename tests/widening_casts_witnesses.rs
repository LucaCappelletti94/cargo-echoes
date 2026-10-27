//! Public relations for independently executed widening cast witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn widening_cast_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/widening_casts_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
