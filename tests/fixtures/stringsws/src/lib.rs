/// ```
/// fn greet(name: &str) -> String {
///     println!("{}, {}", "hello", name);
///     format!("done")
/// }
/// ```
pub struct Arguments;

/// ```
/// fn greet(name: &str) -> String {
///     println!("hello, {name}");
///     "done".to_string()
/// }
/// ```
pub struct Inlined;

/// ```
/// fn greet(name: &str) -> String {
///     println!("hi, {name}");
///     "done".to_string()
/// }
/// ```
pub struct Shorter;

pub fn reset() -> String {
    String::from("")
}

pub fn clear() -> String {
    String::new()
}
