/// ```
/// fn greeting(name: &str) -> String {
///     let mut text = "hello ".to_owned();
///     text.push_str(name);
///     text
/// }
/// ```
pub struct Owned;

/// ```
/// fn greeting(name: &str) -> String {
///     let mut text = String::from("hello ");
///     text.push_str(name);
///     text
/// }
/// ```
pub struct Constructed;

/// ```
/// fn greeting(name: &str) -> String {
///     let mut text = name.to_owned();
///     text.push_str("hello ");
///     text
/// }
/// ```
pub struct Swapped;

pub mod bare;

pub fn empty() -> Vec<u32> {
    let mut values = vec![];
    values.push(1u32);
    values
}

pub fn fresh() -> Vec<u32> {
    let mut values = Vec::new();
    values.push(1u32);
    values
}
