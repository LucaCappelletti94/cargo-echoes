/// ```
/// fn f(x: u32) -> bool { x == x }
/// ```
pub struct Written;

/// ```
/// fn f(x: u32) -> bool { true }
/// ```
pub struct Folded;

/// ```
/// fn f(x: f64) -> bool { x == x }
/// ```
pub struct FloatWritten;

/// ```
/// fn f(x: f64) -> bool { true }
/// ```
pub struct FloatFolded;

/// ```
/// fn f(x: u32) -> bool { (1u32 / x) == (1u32 / x) }
/// ```
pub struct PartialWritten;

/// ```
/// fn f() -> bool {
///     let x = std::hint::black_box(1u32);
///     x == x
/// }
/// ```
pub struct OpaqueRead;

/// ```
/// fn f() -> bool {
///     let x = std::hint::black_box(1u32);
///     true
/// }
/// ```
pub struct OpaqueTrue;

pub fn written(x: u32) -> bool { x == x }
pub fn folded(x: u32) -> bool { true }

pub mod floating {
    pub fn written(x: f64) -> bool { x == x }
    pub fn folded(x: f64) -> bool { true }
}

pub mod partial {
    pub fn written(x: u32) -> bool { (1u32 / x) == (1u32 / x) }
    pub fn folded(x: u32) -> bool { true }
}

pub mod custom {
    pub struct Token;
    impl PartialEq for Token {
        fn eq(&self, _: &Self) -> bool { false }
    }
    pub fn written(x: Token) -> bool { x == x }
    pub fn folded(x: Token) -> bool { true }
}
