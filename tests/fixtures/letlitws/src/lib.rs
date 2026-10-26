/// ```
/// fn f(x: u32) -> u32 { let mut total: u32 = 0; total += x; total }
/// ```
pub struct Annotated;

/// ```
/// fn f(x: u32) -> u32 { let mut total = 0u32; total += x; total }
/// ```
pub struct Suffixed;

/// ```
/// fn f(x: u32) -> u32 { let mut total: u32 = 0x0; total += x; total }
/// ```
pub struct Hexadecimal;

/// ```
/// fn f(x: usize) -> usize { let mut total: usize = 0; total += x; total }
/// ```
pub struct PointerAnnotated;

/// ```
/// fn f(x: usize) -> usize { let mut total = 0usize; total += x; total }
/// ```
pub struct PointerSuffixed;

pub fn annotated(x: u32) -> u32 { let mut total: u32 = 0; total += x; total }
pub fn suffixed(x: u32) -> u32 { let mut total = 0u32; total += x; total }
pub fn hexadecimal(x: u32) -> u32 { let mut total: u32 = 0x0; total += x; total }

pub mod pointer {
    pub fn annotated(x: usize) -> usize { let mut total: usize = 0; total += x; total }
    pub fn suffixed(x: usize) -> usize { let mut total = 0usize; total += x; total }
}
