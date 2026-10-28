//! Public relations for independently executed literal cast, limit and power-of-two witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn literal_casts_limits_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/literal_casts_limits_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
