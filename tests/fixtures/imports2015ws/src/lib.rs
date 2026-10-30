use std::collections;
use std::collections::HashMap;

pub fn imported(words: &[&str]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for word in words {
        *counts.entry(word.to_string()).or_insert(0) += 1;
    }
    counts
}

pub fn module(words: &[&str]) -> collections::HashMap<String, usize> {
    let mut counts = collections::HashMap::new();
    for word in words {
        *counts.entry(word.to_string()).or_insert(0) += 1;
    }
    counts
}

pub fn rooted(words: &[&str]) -> std::collections::HashMap<String, usize> {
    let mut counts = std::collections::HashMap::new();
    for word in words {
        *counts.entry(word.to_string()).or_insert(0) += 1;
    }
    counts
}

pub fn ordered(words: &[&str]) -> collections::BTreeMap<String, usize> {
    let mut counts = collections::BTreeMap::new();
    for word in words {
        *counts.entry(word.to_string()).or_insert(0) += 1;
    }
    counts
}
