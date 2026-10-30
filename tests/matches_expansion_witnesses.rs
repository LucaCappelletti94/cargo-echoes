//! Public relations for independently executed `matches!` expansion witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn matches_expansion_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/matches_expansion_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
