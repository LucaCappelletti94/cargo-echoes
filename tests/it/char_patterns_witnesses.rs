//! Public relations for independently executed one-character pattern witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn char_pattern_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "../fixtures/dependency/char_patterns_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
