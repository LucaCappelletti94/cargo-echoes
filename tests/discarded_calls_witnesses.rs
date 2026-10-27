//! Public relations for independently executed discarded call witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn discarded_call_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/discarded_calls_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
