//! Public relations for independently executed `loop`-`match` and `while let` witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn loop_while_let_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> = serde_json::from_str(include_str!(
        "fixtures/dependency/loop_while_let_cases.json"
    ))
    .unwrap();
    relations(&cases);
}
