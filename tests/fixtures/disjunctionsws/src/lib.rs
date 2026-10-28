/// ```
/// fn check(code: u8, n: u32) -> bool {
///     (code == 97 || code == 101 || code == 105) && n >= 1 && n <= 9
/// }
/// assert!(check(101, 9));
/// ```
pub struct Chained;

/// ```
/// fn check(code: u8, n: u32) -> bool {
///     matches!(code, 105 | 97 | 101) && (1..=9).contains(&n)
/// }
/// assert!(check(101, 9));
/// ```
pub struct Matched;

/// ```
/// fn check(code: u8, n: u32) -> bool {
///     matches!(code, 97 | 101 | 111) && (1..=9).contains(&n)
/// }
/// assert!(check(101, 9));
/// ```
pub struct Other;

pub fn weekend(day: u8) -> bool {
    day == 6 || day == 7
}

pub fn rest(day: u8) -> bool {
    matches!(day, 7 | 6)
}
