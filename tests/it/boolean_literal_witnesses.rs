//! Public relations for independently executed boolean literal comparison witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn boolean_literal_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "../fixtures/dependency/boolean_literal_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
