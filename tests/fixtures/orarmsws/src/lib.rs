/// ```
/// fn f(code: u32, fallback: u32) -> u32 {
///     match code {
///         0 => fallback,
///         1 => fallback,
///         _ => code,
///     }
/// }
/// ```
pub struct Split;

/// ```
/// fn f(code: u32, fallback: u32) -> u32 {
///     match code {
///         1 | 0 => fallback,
///         _ => code,
///     }
/// }
/// ```
pub struct Joined;

/// ```
/// fn f(code: u32, fallback: u32) -> u32 {
///     match code {
///         0 => fallback,
///         2 => code,
///         1 => fallback,
///         _ => code,
///     }
/// }
/// ```
pub struct Apart;

pub fn unwrapped(slot: Option<u32>) -> u32 {
    let Some(value) = slot else {
        return 0;
    };
    println!("found {value}");
    value
}

pub fn matched(slot: Option<u32>) -> u32 {
    match slot {
        Some(value) => {
            println!("found {value}");
            value
        }
        _ => {
            return 0;
        }
    }
}
