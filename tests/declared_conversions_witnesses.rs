//! Public relations for independently executed declared-target conversion witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn declared_conversion_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/declared_conversions_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
