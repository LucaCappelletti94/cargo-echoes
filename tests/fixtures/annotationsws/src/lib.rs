use std::collections::HashMap;

/// ```
/// let mut seen: Vec<u32> = Vec::new();
/// seen.push(4);
/// assert_eq!(seen.len(), 1);
/// ```
pub struct Annotated;

/// ```
/// let mut seen = Vec::<u32>::new();
/// seen.push(4);
/// assert_eq!(seen.len(), 1);
/// ```
pub struct Turbofish;

/// ```
/// let mut seen = Vec::<u64>::new();
/// seen.push(4);
/// assert_eq!(seen.len(), 1);
/// ```
pub struct Wider;

pub fn counts(word: &str) -> usize {
    let mut seen: HashMap<char, u32> = HashMap::new();
    for c in word.chars() {
        *seen.entry(c).or_insert(0) += 1;
    }
    seen.len()
}

pub fn tallies(word: &str) -> usize {
    let mut seen = HashMap::<char, u32>::new();
    for c in word.chars() {
        *seen.entry(c).or_insert(0) += 1;
    }
    seen.len()
}

pub fn bytes(word: &str) -> usize {
    let mut seen = HashMap::<u8, u32>::new();
    for c in word.bytes() {
        *seen.entry(c).or_insert(0) += 1;
    }
    seen.len()
}
