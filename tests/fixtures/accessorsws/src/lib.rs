/// ```
/// fn head(values: &[u32]) -> Option<&u32> {
///     values.get(0)
/// }
/// assert_eq!(head(&[4, 5]), Some(&4));
/// ```
pub struct Indexed;

/// ```
/// fn head(values: &[u32]) -> Option<&u32> {
///     values.first()
/// }
/// assert_eq!(head(&[4, 5]), Some(&4));
/// ```
pub struct First;

/// ```
/// fn head(values: &[u32]) -> Option<&u32> {
///     values.last()
/// }
/// assert_eq!(head(&[4, 5]), Some(&5));
/// ```
pub struct Last;

pub fn blank(text: &str) -> bool {
    text == ""
}

pub fn empty(text: &str) -> bool {
    text.is_empty()
}

pub fn spaced(text: &str) -> bool {
    text == " "
}
