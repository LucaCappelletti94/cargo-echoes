/// ```
/// fn f(p: bool) -> bool { p == false }
/// ```
pub struct Compared;

/// ```
/// fn f(p: bool) -> bool { true != p }
/// ```
pub struct Reversed;

/// ```
/// fn f(p: bool) -> bool { !p }
/// ```
pub struct Negated;

/// ```
/// fn f(p: bool) -> bool { p != false }
/// ```
pub struct Kept;

/// ```
/// fn read() -> bool { println!("read"); true }
/// fn f() -> bool { read() == false }
/// ```
pub struct EffectCompared;

/// ```
/// fn read() -> bool { println!("read"); true }
/// fn f() -> bool { !read() }
/// ```
pub struct EffectNegated;

pub fn compared(p: bool) -> bool { p == false }
pub fn reversed(p: bool) -> bool { true != p }
pub fn negated(p: bool) -> bool { !p }
pub fn kept(p: bool) -> bool { p != false }
pub fn identity(p: bool) -> bool { p }

pub mod effects {
    fn read() -> bool { println!("read"); true }
    pub fn compared() -> bool { read() == false }
    pub fn negated() -> bool { !read() }
}
