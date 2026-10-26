/// ```
/// fn f(x: u64, y: u64, z: u64) -> u64 { x.wrapping_mul(y) ^ z.saturating_add(x) }
/// ```
pub struct Written;

/// ```
/// fn f(x: u64, y: u64, z: u64) -> u64 { x.saturating_add(z) ^ y.wrapping_mul(x) }
/// ```
pub struct Reordered;

/// ```
/// fn f(x: u64, y: u64, z: u64) -> u64 { x.wrapping_sub(y) ^ z.saturating_sub(x) }
/// ```
pub struct Subtracted;

/// ```
/// fn f(x: u64, y: u64, z: u64) -> u64 { x.saturating_sub(z) ^ y.wrapping_sub(x) }
/// ```
pub struct SubtractedReversed;

pub fn product(x: u64, y: u64) -> u64 { x * y }
pub fn reversed_product(x: u64, y: u64) -> u64 { y * x }

pub mod subtraction {
    pub fn difference(x: u64, y: u64) -> u64 { x - y }
    pub fn reversed(x: u64, y: u64) -> u64 { y - x }
}
