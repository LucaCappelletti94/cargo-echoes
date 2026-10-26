/// ```
/// fn f(x: u32) -> u32 { x ^ x }
/// ```
pub struct Written;

/// ```
/// fn f(x: u32) -> u32 { 0u32 }
/// ```
pub struct Zero;

/// ```
/// fn f(x: u32, y: u32) -> u32 { x ^ y ^ x }
/// ```
pub struct Chain;

/// ```
/// fn f(x: u32, y: u32) -> u32 { y }
/// ```
pub struct Remaining;

/// ```
/// fn f(x: u32) -> u32 { x ^ x ^ x }
/// ```
pub struct OddWritten;

/// ```
/// fn f(x: u32) -> u32 { x ^ 0u32 }
/// ```
pub struct LiteralIdentity;

/// ```
/// fn f(x: u32) -> u32 { x }
/// ```
pub struct Plain;

/// ```
/// fn f(p: bool) -> bool { p ^ p }
/// ```
pub struct BoolWritten;

/// ```
/// fn f(p: bool) -> bool { false }
/// ```
pub struct BoolFalse;

/// ```
/// fn f(x: f64) -> u64 { x.to_bits() ^ x.to_bits() }
/// ```
pub struct FloatWritten;

/// ```
/// fn f(x: f64) -> u64 { 0u64 }
/// ```
pub struct FloatZero;

/// ```
/// fn f(x: u32) -> u32 { (1u32 / x) ^ (1u32 / x) }
/// ```
pub struct PartialWritten;

/// ```
/// fn f(x: u32) -> u32 { 0 }
/// ```
pub struct PartialZero;

/// ```
/// fn f() -> u32 {
///     let x = std::hint::black_box(1u32);
///     x ^ x
/// }
/// ```
pub struct OpaqueRead;

/// ```
/// fn f() -> u32 {
///     let x = std::hint::black_box(1u32);
///     0u32
/// }
/// ```
pub struct OpaqueZero;

/// ```
/// fn f(x: u32, y: u32) -> u32 { x ^ y }
/// ```
pub struct Distinct;

pub fn written(x: u32) -> u32 { x ^ x }
pub fn zero(x: u32) -> u32 { 0u32 }
pub fn chain(x: u32, y: u32) -> u32 { x ^ y ^ x }
pub fn remaining(x: u32, y: u32) -> u32 { y }
pub fn odd_written(x: u32) -> u32 { x ^ x ^ x }
pub fn literal_identity(x: u32) -> u32 { x ^ 0u32 }
pub fn plain(x: u32) -> u32 { x }

pub mod boolean {
    pub fn written(p: bool) -> bool { p ^ p }
    pub fn folded(p: bool) -> bool { false }
}

pub mod floating {
    pub fn written(x: f64) -> u64 { x.to_bits() ^ x.to_bits() }
    pub fn folded(x: f64) -> u64 { 0u64 }
}

pub mod partial {
    pub fn written(x: u32) -> u32 { (1u32 / x) ^ (1u32 / x) }
    pub fn folded(x: u32) -> u32 { 0 }
}

pub mod custom {
    #[derive(Clone, Copy)]
    pub struct Token;

    impl core::ops::BitXor for Token {
        type Output = Token;
        fn bitxor(self, _other: Self) -> Self { Token }
    }

    pub fn written(x: Token) -> Token { x ^ x }
    pub fn folded(x: Token) -> Token { Token }
}
