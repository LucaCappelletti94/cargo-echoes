//! Public relations for independently executed constructor return witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn constructor_return_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "../fixtures/dependency/constructor_returns_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
