/// ```
/// fn label(name: &str) -> String {
///     name.to_owned()
/// }
/// assert_eq!(label("echo"), "echo");
/// ```
pub struct Owned;

/// ```
/// fn label(name: &str) -> String {
///     String::from(name)
/// }
/// assert_eq!(label("echo"), "echo");
/// ```
pub struct Converted;

/// ```
/// fn label(name: &str) -> String {
///     format!("{name}")
/// }
/// assert_eq!(label("echo"), "echo");
/// ```
pub struct Formatted;

pub fn cloned(name: &String) -> String {
    name.clone()
}

pub fn viewed(name: &String) -> String {
    String::from(name.as_str())
}

pub fn trimmed(name: &String) -> String {
    String::from(name.trim())
}
