//! Public relations for independently executed boolean integer and `min`/`max` witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn bool_int_min_max_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/bool_int_min_max_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
