//! Public relations for independently executed built-in `concat!` witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn string_literal_concat_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("fixtures/dependency/concat_cases.json")).unwrap();
    relations(&cases);
}
