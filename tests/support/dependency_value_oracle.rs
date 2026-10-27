extern crate alloc;

use alloc::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    Literal(u32),
    Parameter(usize),
    Operation(char, Vec<Self>),
}

pub fn operation(op: char, operands: impl IntoIterator<Item = Value>) -> Value {
    let mut leaves = Vec::new();
    for operand in operands {
        match operand {
            Value::Operation(inner, children) if inner == op => leaves.extend(children),
            other => leaves.push(other),
        }
    }
    if op == '^' {
        xor_form(leaves)
    } else {
        leaves.sort();
        leaves.dedup();
        emit(op, leaves)
    }
}

fn xor_form(leaves: Vec<Value>) -> Value {
    let mut counts = BTreeMap::new();
    for leaf in leaves {
        *counts.entry(leaf).or_insert(0usize) += 1;
    }
    let mut odd: Vec<Value> = counts
        .into_iter()
        .filter_map(|(leaf, count)| (count % 2 == 1).then_some(leaf))
        .collect();
    odd.retain(|leaf| !matches!(leaf, Value::Literal(0)));
    if odd.is_empty() {
        Value::Literal(0)
    } else {
        emit('^', odd)
    }
}

fn emit(op: char, mut leaves: Vec<Value>) -> Value {
    let mut literal = None;
    leaves.retain(|leaf| {
        let Value::Literal(value) = leaf else {
            return true;
        };
        literal = Some(match literal {
            None => *value,
            Some(left) => match op {
                '&' => left & value,
                '|' => left | value,
                '^' => left ^ value,
                _ => panic!("unsupported oracle operation"),
            },
        });
        false
    });
    let (identity, absorbing) = match op {
        '&' => (u32::MAX, Some(0)),
        '|' => (0, Some(u32::MAX)),
        _ => (0, None),
    };
    if let Some(value) = literal {
        if Some(value) == absorbing {
            return Value::Literal(value);
        }
        if value != identity {
            leaves.push(Value::Literal(value));
        }
    }
    leaves.sort();
    match leaves.len() {
        0 => Value::Literal(identity),
        1 => leaves.pop().unwrap(),
        _ => Value::Operation(op, leaves),
    }
}
