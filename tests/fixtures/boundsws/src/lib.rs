/// ```
/// fn busy(queued: u32, limit: u32) -> bool {
///     queued >= 1 && limit <= 9
/// }
/// ```
pub struct Inclusive;

/// ```
/// fn busy(queued: u32, limit: u32) -> bool {
///     queued != 0 && limit < 10
/// }
/// ```
pub struct Strict;

/// ```
/// fn busy(queued: u32, limit: u32) -> bool {
///     queued != 0 && limit < 9
/// }
/// ```
pub struct Tighter;

pub fn adult(age: u8) -> bool {
    age >= 18
}

pub fn grown(age: u8) -> bool {
    age > 17
}
