/// ```
/// fn lower(c: char) -> bool {
///     c.is_ascii_lowercase()
/// }
/// assert!(lower('q') && !lower('Q'));
/// ```
pub struct Predicate;

/// ```
/// fn lower(c: char) -> bool {
///     c >= 'a' && c <= 'z'
/// }
/// assert!(lower('q') && !lower('Q'));
/// ```
pub struct Bounds;

/// ```
/// fn lower(c: char) -> bool {
///     c.is_ascii_uppercase()
/// }
/// assert!(!lower('q') && lower('Q'));
/// ```
pub struct Upper;

pub fn after(c: char) -> bool {
    c > 'm'
}

pub fn from(c: char) -> bool {
    c >= 'n'
}

pub fn including(c: char) -> bool {
    c >= 'm'
}
