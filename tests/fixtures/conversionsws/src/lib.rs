/// ```
/// fn widen(x: u16) -> u32 {
///     x.into()
/// }
/// assert_eq!(widen(7), 7);
/// ```
pub struct Into;

/// ```
/// fn widen(x: u16) -> u32 {
///     u32::from(x)
/// }
/// assert_eq!(widen(7), 7);
/// ```
pub struct From;

/// ```
/// fn widen(x: u16) -> u32 {
///     u32::from(x) + 1
/// }
/// assert_eq!(widen(7), 8);
/// ```
pub struct Shifted;

pub fn drained(items: &mut Vec<u32>) -> Vec<u32> {
    std::mem::replace(items, Vec::new())
}

pub fn taken(items: &mut Vec<u32>) -> Vec<u32> {
    std::mem::take(items)
}

pub fn swapped(items: &mut Vec<u32>) -> Vec<u32> {
    std::mem::replace(items, vec![0])
}
