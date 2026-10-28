//! Public relations for independently executed guard clause witnesses.

#[path = "support/witnesses.rs"]
mod witnesses;

use witnesses::relations;

#[test]
fn guard_clause_sources_preserve_their_public_relations() {
    let cases: Vec<witnesses::Case> =
        serde_json::from_str(include_str!("fixtures/dependency/guard_clauses_cases.json")).unwrap();
    relations(&cases);
}
