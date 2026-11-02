//! Public relations for independently executed primitive XOR witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn native_xor_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("../fixtures/dependency/xor_cases.json")).unwrap();
    relations(&cases);
}
