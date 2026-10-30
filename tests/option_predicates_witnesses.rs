//! Public relations for independently executed `Option` and `Result` predicate witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn option_predicate_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/option_predicates_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
