//! Public relations for independently executed return-type literal typing witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn return_literal_typing_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/return_literal_typing_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
