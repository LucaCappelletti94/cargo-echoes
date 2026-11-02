//! Public relations for independently executed owned string and empty vector witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn owned_string_and_empty_vector_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "../fixtures/dependency/prelude_owned_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
