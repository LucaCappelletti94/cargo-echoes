/// ```
/// fn same(left: u32, right: u32) -> bool {
///     &left.clone() == &right.clone()
/// }
/// ```
pub struct Cloned;

/// ```
/// fn same(left: u32, right: u32) -> bool {
///     left == right
/// }
/// ```
pub struct Plain;

/// ```
/// fn same(left: u32, right: u32) -> bool {
///     left <= right
/// }
/// ```
pub struct Ordered;

pub fn mixed(count: u32, flag: bool) -> u32 {
    if flag.clone() { *&count } else { 0u32 }
}

pub fn direct(count: u32, flag: bool) -> u32 {
    if flag { count } else { 0u32 }
}
