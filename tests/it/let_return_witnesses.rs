//! Public relations for independently executed let-and-return witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn let_return_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("../fixtures/dependency/let_return_cases.json")).unwrap();
    relations(&cases);
}
