//! Public relations for independently executed integer identity witnesses.

use crate::support::witnesses;

use witnesses::relations;

#[test]
fn identity_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("../fixtures/dependency/identities_cases.json")).unwrap();
    relations(&cases);
}
