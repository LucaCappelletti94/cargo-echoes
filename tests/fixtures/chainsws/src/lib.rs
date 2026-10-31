/// ```
/// let v: Vec<u32> = vec![3, 1, 2];
/// let w: Vec<u32> = v.iter().map(|x| *x).collect();
/// assert_eq!(w, [3, 1, 2]);
/// ```
pub struct Mapped;

/// ```
/// let v: Vec<u32> = vec![3, 1, 2];
/// let w = v.iter().copied().collect::<Vec<u32>>();
/// assert_eq!(w, [3, 1, 2]);
/// ```
pub struct Copied;

/// ```
/// let v: Vec<u32> = vec![3, 1, 2];
/// let w = v.iter().rev().copied().collect::<Vec<u32>>();
/// assert_eq!(w, [2, 1, 3]);
/// ```
pub struct Reversed;

pub fn quiet(words: &[u32]) -> bool {
    !words.iter().any(|w| *w > 9)
}

pub fn small(words: &[u32]) -> bool {
    words.iter().all(|w| !(*w > 9))
}

pub fn loud(words: &[u32]) -> bool {
    words.iter().all(|w| *w > 9)
}
