/// ```
/// fn bucket(id: u32) -> u32 {
///     id / 16 % 8
/// }
/// fn limit() -> u8 {
///     u8::MAX
/// }
/// assert_eq!(bucket(255), 7);
/// assert_eq!(limit(), 255);
/// ```
pub struct Divided;

/// ```
/// fn bucket(id: u32) -> u32 {
///     (id >> 4) & 7
/// }
/// fn limit() -> u8 {
///     255u8
/// }
/// assert_eq!(bucket(255), 7);
/// assert_eq!(limit(), 255);
/// ```
pub struct Shifted;

/// ```
/// fn bucket(id: u32) -> u32 {
///     id / 16 % 4
/// }
/// fn limit() -> u8 {
///     u8::MAX
/// }
/// assert_eq!(bucket(255), 3);
/// assert_eq!(limit(), 255);
/// ```
pub struct Narrower;

pub fn is_even(n: i64) -> bool {
    n % 2 == 0
}

pub fn low_bit_clear(n: i64) -> bool {
    n & 1 == 0
}
