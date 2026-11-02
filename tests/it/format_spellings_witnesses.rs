//! Public relations for independently executed formatting macro spelling witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn format_spelling_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "../fixtures/dependency/format_spellings_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
