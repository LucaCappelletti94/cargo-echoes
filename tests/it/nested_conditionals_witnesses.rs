//! Public relations for independently executed nested conditional witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn nested_conditional_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "../fixtures/dependency/nested_conditionals_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
