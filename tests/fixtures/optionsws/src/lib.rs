/// ```
/// fn missing(slot: Option<u32>) -> bool {
///     slot.is_none()
/// }
/// assert!(missing(None) && !missing(Some(1)));
/// ```
pub struct Method;

/// ```
/// fn missing(slot: Option<u32>) -> bool {
///     slot == None
/// }
/// assert!(missing(None) && !missing(Some(1)));
/// ```
pub struct Compared;

/// ```
/// fn missing(slot: Option<u32>) -> bool {
///     slot.is_some()
/// }
/// assert!(!missing(None) && missing(Some(1)));
/// ```
pub struct Present;

pub fn limit(slot: Option<u32>) -> u32 {
    slot.unwrap_or(10)
}

pub fn matched(slot: Option<u32>) -> u32 {
    match slot {
        Some(value) => value,
        None => 10,
    }
}

pub fn wider(slot: Option<u32>) -> u32 {
    slot.unwrap_or(20)
}
