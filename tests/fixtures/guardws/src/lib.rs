/// ```
/// fn report(total: u32, quiet: bool) {
///     if quiet {
///         return;
///     }
///     println!("{total}");
///     println!("done");
/// }
/// ```
pub struct Guarded;

/// ```
/// fn report(total: u32, quiet: bool) {
///     if !quiet {
///         println!("{total}");
///         println!("done");
///     }
/// }
/// ```
pub struct Nested;

/// ```
/// fn report(total: u32, quiet: bool) {
///     if quiet {
///         println!("quiet");
///     }
///     println!("{total}");
///     println!("done");
/// }
/// ```
pub struct Unguarded;

pub fn drain(mut pending: u32, ready: bool) {
    loop {
        if ready {
            break;
        }
        pending -= 1;
        println!("{pending}");
    }
}

pub fn wait(mut pending: u32, ready: bool) {
    while !ready {
        pending -= 1;
        println!("{pending}");
    }
}
