//! Public relations for independently executed `&&` and `||` witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn boolean_logic_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("fixtures/dependency/boolean_logic_cases.json")).unwrap();
    relations(&cases);
}
