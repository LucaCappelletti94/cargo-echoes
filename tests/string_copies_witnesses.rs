//! Public relations for independently executed owned string copy witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn string_copy_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("fixtures/dependency/string_copies_cases.json")).unwrap();
    relations(&cases);
}
