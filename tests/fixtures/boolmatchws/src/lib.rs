/// ```
/// fn f(p: bool, x: u32, y: u32) -> u32 { match p { true => x, false => y } }
/// ```
pub struct Matched;

/// ```
/// fn f(p: bool, x: u32, y: u32) -> u32 { match p { false => y, true => x } }
/// ```
pub struct Reversed;

/// ```
/// fn f(p: bool, x: u32, y: u32) -> u32 { if p { x } else { y } }
/// ```
pub struct Conditional;

/// ```
/// fn f(p: bool, x: u32, y: u32) -> u32 { match p { true => x, _ => y } }
/// ```
pub struct Wildcard;

/// ```
/// fn read() -> u32 { println!("read"); 7 }
/// fn f(p: bool, y: u32) -> u32 { match p { true => read(), false => y } }
/// ```
pub struct EffectMatched;

/// ```
/// fn read() -> u32 { println!("read"); 7 }
/// fn f(p: bool, y: u32) -> u32 { if p { read() } else { y } }
/// ```
pub struct EffectConditional;

/// ```
/// use std::cell::RefCell;
/// fn f(c: &RefCell<bool>, x: u32, y: u32) -> u32 { match *c.borrow() { true => x, false => y } }
/// ```
pub struct TemporaryMatched;

/// ```
/// use std::cell::RefCell;
/// fn f(c: &RefCell<bool>, x: u32, y: u32) -> u32 { if *c.borrow() { x } else { y } }
/// ```
pub struct TemporaryConditional;

pub fn matched(p: bool, x: u32, y: u32) -> u32 { match p { true => x, false => y } }
pub fn reversed(p: bool, x: u32, y: u32) -> u32 { match p { false => y, true => x } }
pub fn conditional(p: bool, x: u32, y: u32) -> u32 { if p { x } else { y } }
pub fn wildcard(p: bool, x: u32, y: u32) -> u32 { match p { true => x, _ => y } }

pub mod effects {
    fn read() -> u32 { println!("read"); 7 }
    pub fn matched(p: bool, y: u32) -> u32 { match p { true => read(), false => y } }
    pub fn conditional(p: bool, y: u32) -> u32 { if p { read() } else { y } }
}
