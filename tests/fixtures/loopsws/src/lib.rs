/// ```
/// let mut words = "a b c".split(' ');
/// let mut count = 0;
/// loop {
///     match words.next() {
///         Some(_) => count += 1,
///         None => break,
///     }
/// }
/// assert_eq!(count, 3);
/// ```
pub struct Looped;

/// ```
/// let mut words = "a b c".split(' ');
/// let mut count = 0;
/// while let Some(_) = words.next() {
///     count += 1
/// }
/// assert_eq!(count, 3);
/// ```
pub struct WhileLet;

/// ```
/// let mut words = "a b c".split(' ');
/// let mut count = 0;
/// while let Some(_) = words.next() {
///     count += 2
/// }
/// assert_eq!(count, 6);
/// ```
pub struct Doubled;

pub fn drain(items: &mut Vec<u32>) -> u32 {
    let mut total = 0;
    loop {
        match items.pop() {
            Some(item) => total += item,
            None => break,
        }
    }
    total
}

pub fn pop_all(items: &mut Vec<u32>) -> u32 {
    let mut total = 0;
    while let Some(item) = items.pop() {
        total += item
    }
    total
}

pub fn pop_twice(items: &mut Vec<u32>) -> u32 {
    let mut total = 0;
    while let Some(item) = items.pop() {
        total += item * 2
    }
    total
}
