/// ```
/// fn total(values: &[u32]) -> u32 {
///     let mut sum = 0;
///     for value in values.iter() {
///         sum += value;
///     }
///     sum
/// }
/// ```
pub struct Iterated;

/// ```
/// fn total(values: &[u32]) -> u32 {
///     let mut sum = 0;
///     for value in values {
///         sum += value;
///     }
///     sum
/// }
/// ```
pub struct Borrowed;

/// ```
/// fn total(values: &[u32]) -> u32 {
///     let mut sum = 1;
///     for value in values {
///         sum += value;
///     }
///     sum
/// }
/// ```
pub struct Offset;

pub fn blank(name: &str, tags: &[u8]) -> bool {
    name.len() == 0 && tags.len() < 1
}

pub fn unset(name: &str, tags: &[u8]) -> bool {
    name.is_empty() && tags.is_empty()
}
