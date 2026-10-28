//! Public relations for independently executed owned string and formatting witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn strings_format_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/strings_format_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
