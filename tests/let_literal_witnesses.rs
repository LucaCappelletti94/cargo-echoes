//! Public relations for independently executed `let` literal annotation witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn let_literal_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("fixtures/dependency/let_literal_cases.json")).unwrap();
    relations(&cases);
}
