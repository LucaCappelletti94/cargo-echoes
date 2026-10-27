/// ```
/// fn total(count: u32, extra: u64) -> u64 {
///     extra + count as u64
/// }
/// ```
pub struct Cast;

/// ```
/// fn total(count: u32, extra: u64) -> u64 {
///     extra + u64::from(count)
/// }
/// ```
pub struct Converted;

/// ```
/// fn total(count: u64, extra: u64) -> u64 {
///     extra + count as u32 as u64
/// }
/// ```
pub struct Truncated;

pub fn widened(flag: bool, count: u16) -> i64 {
    i64::from(count) + flag as i64
}

pub fn converted(flag: bool, count: u16) -> i64 {
    count as i64 + i64::from(flag)
}
