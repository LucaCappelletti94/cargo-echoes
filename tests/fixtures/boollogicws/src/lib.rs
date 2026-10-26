/// ```
/// fn f(p: bool, q: bool) -> bool { if p { q } else { false } }
/// ```
pub struct Conditional;

/// ```
/// fn f(p: bool, q: bool) -> bool { q && p }
/// ```
pub struct Commuted;

/// ```
/// fn f(p: bool, q: bool) -> bool { p && q && true && p }
/// ```
pub struct Padded;

/// ```
/// fn f(p: bool, q: bool) -> bool { p || q }
/// ```
pub struct Disjunction;

/// ```
/// fn read() -> bool { println!("read"); true }
/// fn f(p: bool) -> bool { read() && p }
/// ```
pub struct EffectFirst;

/// ```
/// fn read() -> bool { println!("read"); true }
/// fn f(p: bool) -> bool { p && read() }
/// ```
pub struct EffectLast;

pub fn conditional(p: bool, q: bool) -> bool { if p { q } else { false } }
pub fn commuted(p: bool, q: bool) -> bool { q && p }
pub fn padded(p: bool, q: bool) -> bool { p && q && true && p }
pub fn disjunction(p: bool, q: bool) -> bool { p || q }

pub mod effects {
    fn read() -> bool { println!("read"); true }
    pub fn first(p: bool) -> bool { read() && p }
    pub fn last(p: bool) -> bool { p && read() }
}
