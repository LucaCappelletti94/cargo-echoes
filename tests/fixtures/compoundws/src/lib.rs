/// ```
/// fn f(mut x: u32, y: u32) -> u32 { x ^= y; x += y; x }
/// ```
pub struct Compound;

/// ```
/// fn f(mut x: u32, y: u32) -> u32 { x = x ^ y; x = x + y; x }
/// ```
pub struct Expanded;

/// ```
/// fn f(mut x: u32, y: u32) -> u32 { x = y ^ x; x = x + y; x }
/// ```
pub struct Reordered;

/// ```
/// struct S { x: u32 }
/// fn f(mut s: S, y: u32) -> u32 { s.x += y; s.x }
/// ```
pub struct FieldCompound;

/// ```
/// struct S { x: u32 }
/// fn f(mut s: S, y: u32) -> u32 { s.x = s.x + y; s.x }
/// ```
pub struct FieldExpanded;

pub fn compound(mut x: u32, y: u32) -> u32 { x ^= y; x += y; x }
pub fn expanded(mut x: u32, y: u32) -> u32 { x = x ^ y; x = x + y; x }
pub fn reordered(mut x: u32, y: u32) -> u32 { x = y ^ x; x = x + y; x }

pub mod fields {
    pub struct S { pub x: u32 }
    pub fn compound(mut s: S, y: u32) -> u32 { s.x += y; s.x }
    pub fn expanded(mut s: S, y: u32) -> u32 { s.x = s.x + y; s.x }
}
