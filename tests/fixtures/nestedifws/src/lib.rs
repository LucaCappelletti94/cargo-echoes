/// ```
/// fn f(ready: bool, armed: bool, hits: u32) -> u32 {
///     if ready { if armed { hits + 1 } else { hits } } else { hits }
/// }
/// ```
pub struct Nested;

/// ```
/// fn f(ready: bool, armed: bool, hits: u32) -> u32 {
///     if armed && ready { hits + 1 } else { hits }
/// }
/// ```
pub struct Joined;

/// ```
/// fn f(ready: bool, armed: bool, hits: u32) -> u32 {
///     if ready { if armed { hits + 1 } else { hits } } else { 0 }
/// }
/// ```
pub struct Fallback;

pub fn nested(ready: bool, armed: bool) {
    if ready {
        if armed {
            println!("fire");
        }
    }
}

pub fn joined(ready: bool, armed: bool) {
    if ready && armed {
        println!("fire");
    }
}

pub fn selected(ready: bool, hits: u32) -> u32 {
    match ready {
        true => hits,
        _ => 0u32,
    }
}

pub fn conditional(ready: bool, hits: u32) -> u32 {
    if ready { hits } else { 0u32 }
}
