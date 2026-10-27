//! Public relations for independently executed match arm witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn match_arm_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("fixtures/dependency/match_arms_cases.json")).unwrap();
    relations(&cases);
}
