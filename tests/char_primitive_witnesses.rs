//! Public relations for independently executed `char` primitive witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn char_primitive_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/char_primitive_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
