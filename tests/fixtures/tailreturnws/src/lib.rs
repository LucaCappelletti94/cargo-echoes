pub struct D(&'static str);

impl D {
    pub fn value(&self) -> u32 {
        7
    }
}

impl Drop for D {
    fn drop(&mut self) {
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout(), "drop {}", self.0);
    }
}

/// ```
/// struct D(&'static str);
/// impl D {
///     fn value(&self) -> u32 {
///         7
///     }
/// }
/// impl Drop for D {
///     fn drop(&mut self) {
///         use std::io::Write as _;
///         let _ = writeln!(std::io::stdout(), "drop {}", self.0);
///     }
/// }
/// fn guard() -> u32 {
///     return D("temp").value();
/// }
/// fn main() {
///     println!("{}", guard());
/// }
/// ```
struct GuardReturn;

/// ```
/// struct D(&'static str);
/// impl D {
///     fn value(&self) -> u32 {
///         7
///     }
/// }
/// impl Drop for D {
///     fn drop(&mut self) {
///         use std::io::Write as _;
///         let _ = writeln!(std::io::stdout(), "drop {}", self.0);
///     }
/// }
/// fn guard() -> u32 {
///     D("temp").value()
/// }
/// fn main() {
///     println!("{}", guard());
/// }
/// ```
struct GuardTail;

/// ```
/// fn guard() -> u32 {
///     return 1 + 1;
/// }
/// fn main() {
///     println!("{}", guard());
/// }
/// ```
struct PlainReturn;

/// ```
/// fn guard() -> u32 {
///     1 + 1
/// }
/// fn main() {
///     println!("{}", guard());
/// }
/// ```
struct PlainTail;

fn first() -> u32 {
    return D("temp").value();
}

fn second() -> u32 {
    D("temp").value()
}

fn equal_first() -> u32 {
    return 1 + 1;
}

fn equal_second() -> u32 {
    1 + 1
}
