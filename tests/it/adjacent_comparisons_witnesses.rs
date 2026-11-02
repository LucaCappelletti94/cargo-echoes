//! Public relations for independently executed adjacent literal comparison witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn adjacent_comparison_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "../fixtures/dependency/adjacent_comparisons_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
