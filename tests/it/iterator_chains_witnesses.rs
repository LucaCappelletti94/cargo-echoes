//! Public relations for independently executed iterator chain witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn iterator_chain_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "../fixtures/dependency/iterator_chains_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
