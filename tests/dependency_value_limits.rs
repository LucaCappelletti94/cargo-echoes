//! Atomic source and expanded-expression limits through public comparison.

fn binary_tree(op: &str, leaf: &str, count: usize) -> String {
    if count == 1 {
        return leaf.to_owned();
    }
    format!(
        "({}{op}{})",
        binary_tree(op, leaf, count / 2),
        binary_tree(op, leaf, count - count / 2)
    )
}

fn form(source: &str) -> syn_canon::CanonicalForm {
    syn_canon::canonicalize(syn::parse_str(source).unwrap())
}

#[test]
fn expanded_node_limit_admits_the_exact_complete_region() {
    let original = format!(
        "fn f(x:bool)->bool{{let t=x^true;{}}}",
        binary_tree("!=", "t", 4096)
    );
    let inline = format!(
        "fn f(x:bool)->bool{{{}}}",
        binary_tree("!=", "(x^true)", 4096)
    );
    let left = form(&original);
    let right = form(&inline);
    assert_eq!(left, right);
    assert_eq!(left.body_units(), 16_383);
    assert_eq!(right.body_units(), 16_383);
    assert_ne!(
        syn_canon::canonicalize_failing(syn::parse_str(&original).unwrap()),
        syn_canon::canonicalize_failing(syn::parse_str(&inline).unwrap())
    );
}

#[test]
fn expanded_node_exhaustion_retains_the_complete_region() {
    let original = format!(
        "fn f(x:bool)->bool{{let t=x^true;{}}}",
        binary_tree("!=", "t", 4097)
    );
    let inline = format!(
        "fn f(x:bool)->bool{{{}}}",
        binary_tree("!=", "(x^true)", 4097)
    );
    let retained = form(&original);
    assert_ne!(retained, form(&inline));
    assert_eq!(retained.body_units(), 8_200);
}

fn shifts(leaf: &str, count: usize) -> String {
    let mut expression = leaf.to_owned();
    for _ in 0..count {
        expression = format!("({expression}>>1)");
    }
    expression
}

#[test]
fn expanded_expression_limit_keeps_every_original_binding_on_exhaustion() {
    for (count, admitted) in [(62, true), (63, false)] {
        let original = format!("fn f(x:u8)->u8{{let t=x>>1;{}}}", shifts("t", count));
        let inline = format!("fn f(x:u8)->u8{{{}}}", shifts("x", count + 1));
        let left = form(&original);
        let right = form(&inline);
        if admitted {
            assert_eq!(left, right);
            assert_eq!(left.body_units(), 127);
        } else {
            assert_ne!(left, right);
            assert_eq!(left.body_units(), 134);
        }
    }
}

#[test]
fn inherited_boolean_expansion_exhaustion_restores_every_value_binding() {
    let operands = binary_tree("&&", "t", 8190);
    let original = format!("fn f(p:bool)->bool{{let t=p;!({operands})}}");
    let inline = format!("fn f(p:bool)->bool{{!({})}}", operands.replace('t', "p"));
    let retained = form(&original);
    assert_ne!(retained, form(&inline));
    assert_eq!(retained.body_units(), 16_385);
}

#[test]
fn final_boolean_syntax_exhaustion_retains_the_complete_value_region() {
    let operands = binary_tree("&&", "t", 5462).replacen('t', "!t", 1);
    let original = format!("fn f(p:bool)->bool{{let t=p;!({operands})}}");
    let inline = format!("fn f(p:bool)->bool{{!({})}}", operands.replace('t', "p"));
    let retained = form(&original);
    assert_ne!(retained, form(&inline));
    assert_eq!(retained.body_units(), 10_930);
}
