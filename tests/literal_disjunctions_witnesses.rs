//! Public relations for independently executed `matches!` and literal disjunction witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn literal_disjunctions_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/literal_disjunctions_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
