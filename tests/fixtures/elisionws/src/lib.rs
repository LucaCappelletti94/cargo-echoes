/// ```
/// fn first<'a>(words: &'a [String]) -> &'a str {
///     words.first().map_or("", |word| word.as_str())
/// }
/// ```
pub struct Named;

/// ```
/// fn first(words: &[String]) -> &str {
///     words.first().map_or("", |word| word.as_str())
/// }
/// ```
pub struct Elided;

/// ```
/// fn first<'a>(words: &'a [String], fallback: &'a str) -> &'a str {
///     words.first().map_or(fallback, |word| word.as_str())
/// }
/// ```
pub struct Shared;

pub fn largest<T>(values: &[T]) -> Option<T>
where
    T: Ord + Copy,
{
    values.iter().max().copied()
}

pub fn biggest<T: Copy + Ord>(values: &[T]) -> Option<T> {
    values.iter().max().copied()
}
