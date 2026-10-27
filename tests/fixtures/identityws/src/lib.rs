/// ```
/// fn f(offset: u32, mask: u32) -> u32 { (offset + 0) ^ (mask | 0) ^ (offset & mask) }
/// ```
pub struct Padded;

/// ```
/// fn f(offset: u32, mask: u32) -> u32 { offset ^ mask ^ (mask & offset) }
/// ```
pub struct Plain;

/// ```
/// fn f(offset: u32, mask: u32) -> u32 { (offset * 1) ^ mask.wrapping_add(0) ^ (offset & mask) }
/// ```
pub struct Scaled;

/// ```
/// fn f(offset: u32, mask: u32) -> u32 { (offset * 2) ^ mask ^ (offset & mask) }
/// ```
pub struct Doubled;

pub fn padded(offset: u32, mask: u32) -> u32 { (offset + 0) ^ (mask | 0) }
pub fn plain(offset: u32, mask: u32) -> u32 { offset ^ mask }
pub fn scaled(offset: u32, mask: u32) -> u32 { (offset * 1) ^ mask.wrapping_add(0) }

pub mod other {
    pub fn doubled(offset: u32, mask: u32) -> u32 { (offset * 2) ^ mask }
}
