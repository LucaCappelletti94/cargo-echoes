/// ```
/// fn area(width: u32, height: u32) -> u32 {
///     let area = width * height;
///     area
/// }
/// assert_eq!(area(2, 3), 6);
/// ```
pub struct Bound;

/// ```
/// fn area(width: u32, height: u32) -> u32 {
///     width * height
/// }
/// assert_eq!(area(2, 3), 6);
/// ```
pub struct Direct;

/// ```
/// fn area(width: u32, height: u32) -> u32 {
///     width + height
/// }
/// assert_eq!(area(2, 3), 5);
/// ```
pub struct Other;

pub fn scaled(value: u32) -> u32 {
    let scaled = value * 3 + 1;
    scaled
}

pub fn returned(value: u32) -> u32 {
    let scaled = value * 3 + 1;
    return scaled;
}

pub fn direct(value: u32) -> u32 {
    value * 3 + 1
}

pub fn shifted(value: u32) -> u32 {
    value * 3 + 2
}
