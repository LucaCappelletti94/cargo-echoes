/// ```
/// fn refill(values: &mut Vec<u32>) {
///     let _ = values.pop();
///     values.push(1);
/// }
/// ```
pub struct Bound;

/// ```
/// fn refill(values: &mut Vec<u32>) {
///     values.pop();
///     values.push(1);
/// }
/// ```
pub struct Dropped;

/// ```
/// fn refill(values: &mut Vec<u32>) {
///     let _last = values.pop();
///     values.push(1);
/// }
/// ```
pub struct Kept;

pub fn trim(values: &mut Vec<u32>) {
    let _ = values.pop();
    let _ = values.pop();
}

pub fn shorten(values: &mut Vec<u32>) {
    values.pop();
    values.pop();
}
