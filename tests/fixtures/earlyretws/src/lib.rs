/// ```
/// fn f(ready: bool, hits: u32, misses: u32) -> u32 {
///     if ready {
///         return hits;
///     }
///     misses
/// }
/// ```
pub struct Early;

/// ```
/// fn f(ready: bool, hits: u32, misses: u32) -> u32 {
///     if ready { hits } else { misses }
/// }
/// ```
pub struct Branched;

/// ```
/// fn f(ready: bool, hits: u32, misses: u32) -> u32 {
///     if ready {
///         return misses;
///     }
///     hits
/// }
/// ```
pub struct Swapped;

pub fn early(ready: bool, hits: u32) -> u32 {
    if ready {
        println!("ready");
        return hits;
    }
    0u32
}

pub fn branched(ready: bool, hits: u32) -> u32 {
    if ready {
        println!("ready");
        hits
    } else {
        0u32
    }
}

pub fn chained(ready: bool, armed: bool, hits: u32) -> u32 {
    if ready {
        return hits;
    }
    if armed {
        return 2u32;
    }
    0u32
}

pub fn ladder(ready: bool, armed: bool, hits: u32) -> u32 {
    if ready {
        hits
    } else if armed {
        2u32
    } else {
        0u32
    }
}
