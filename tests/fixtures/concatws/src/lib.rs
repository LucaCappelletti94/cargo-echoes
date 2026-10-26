/// ```
/// fn f() -> &'static str { concat!("a", "b") }
/// ```
pub struct Concatenated;

/// ```
/// fn f() -> &'static str { concat!(r"a", "\x62",) }
/// ```
pub struct Escaped;

/// ```
/// fn f() -> &'static str { "ab" }
/// ```
pub struct Literal;

/// ```
/// fn f() -> &'static str { concat!("a", 1) }
/// ```
pub struct Numeric;

/// ```
/// fn f() -> &'static str { "a1" }
/// ```
pub struct NumericLiteral;

/// ```
/// macro_rules! concat { ($($t:tt)*) => { "x" } }
/// fn f() -> &'static str { concat!("a", "b") }
/// ```
pub struct Shadowed;

/// ```
/// macro_rules! concat { ($($t:tt)*) => { "x" } }
/// fn f() -> &'static str { "ab" }
/// ```
pub struct ShadowedLiteral;

pub fn concatenated() -> &'static str { concat!("a", "b") }
pub fn escaped() -> &'static str { concat!(r"a", "\x62",) }
pub fn literal() -> &'static str { "ab" }
pub fn numeric() -> &'static str { concat!("a", 1) }
pub fn numeric_literal() -> &'static str { "a1" }
