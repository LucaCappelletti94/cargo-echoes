//! Public relations for independently executed collection iteration witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn collection_iteration_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/collection_iteration_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
