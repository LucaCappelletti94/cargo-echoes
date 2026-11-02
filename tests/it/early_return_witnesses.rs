//! Public relations for independently executed early-return witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn early_return_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "../fixtures/dependency/early_return_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
