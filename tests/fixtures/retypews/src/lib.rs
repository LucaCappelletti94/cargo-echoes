/// ```
/// fn code(ok: bool) -> u8 {
///     if !ok {
///         return 255;
///     }
///     0
/// }
/// assert_eq!(code(false), 255);
/// ```
pub struct Unsuffixed;

/// ```
/// fn code(ok: bool) -> u8 {
///     if !ok {
///         return 255u8;
///     }
///     0u8
/// }
/// assert_eq!(code(false), 255);
/// ```
pub struct Suffixed;

/// ```
/// fn code(ok: bool) -> u8 {
///     if !ok {
///         return 254;
///     }
///     0
/// }
/// assert_eq!(code(false), 254);
/// ```
pub struct Other;

pub fn limit() -> u32 {
    60 * 60
}

pub fn hour() -> u32 {
    3600u32
}
