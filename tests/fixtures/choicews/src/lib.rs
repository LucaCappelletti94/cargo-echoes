/// ```
/// fn score(won: bool, bonus: u32, base: u32) -> u32 {
///     let point: u32 = if won { 1 } else { 0 };
///     point + if bonus > base { bonus } else { base }
/// }
/// ```
pub struct Branched;

/// ```
/// fn score(won: bool, bonus: u32, base: u32) -> u32 {
///     let point: u32 = won as u32;
///     point + base.max(bonus)
/// }
/// ```
pub struct Folded;

/// ```
/// fn score(won: bool, bonus: u32, base: u32) -> u32 {
///     let point: u32 = won as u32;
///     point + base.min(bonus)
/// }
/// ```
pub struct Smaller;

pub fn ceiling(left: u64, right: u64) -> u64 {
    if left >= right { left } else { right }
}

pub fn larger(left: u64, right: u64) -> u64 {
    std::cmp::max(right, left)
}
