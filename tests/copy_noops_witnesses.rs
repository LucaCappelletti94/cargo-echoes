//! Public relations for independently executed primitive copy witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn primitive_copy_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("fixtures/dependency/copy_noops_cases.json")).unwrap();
    relations(&cases);
}
