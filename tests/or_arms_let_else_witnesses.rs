//! Public relations for independently executed or-pattern arm and `let`-`else` witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn or_arm_and_let_else_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/or_arms_let_else_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
