//! Public relations for independently executed boolean match witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn boolean_match_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "../fixtures/dependency/boolean_match_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
