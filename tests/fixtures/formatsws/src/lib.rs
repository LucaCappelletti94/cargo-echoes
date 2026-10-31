/// ```
/// let total = 3;
/// print!("total {}\n", total);
/// ```
pub struct Printed;

/// ```
/// let total = 3;
/// println!("total {}", total);
/// ```
pub struct Line;

/// ```
/// let total = 3;
/// println!("sum {}", total);
/// ```
pub struct Other;

pub fn labelled(name: &str, count: u32) -> String {
    format!("{0}: {1} {0}", name, count)
}

pub fn inline(name: &str, count: u32) -> String {
    format!("{name}: {count} {name}")
}

pub fn swapped(name: &str, count: u32) -> String {
    format!("{count}: {name} {name}")
}
