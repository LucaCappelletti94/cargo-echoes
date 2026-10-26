//! Public relations for independently executed compound assignment witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn compound_assignment_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/compound_assignment_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
