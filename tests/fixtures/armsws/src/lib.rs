/// ```
/// fn total(count: Option<u32>) -> u32 {
///     if let Some(value) = count {
///         value
///     } else {
///         0
///     }
/// }
/// ```
pub struct IfLet;

/// ```
/// fn total(count: Option<u32>) -> u32 {
///     match count {
///         None => 0,
///         Some(value) => value,
///     }
/// }
/// ```
pub struct Matched;

/// ```
/// fn total(count: Option<u32>) -> u32 {
///     match count {
///         None => 1,
///         Some(value) => value,
///     }
/// }
/// ```
pub struct Shifted;

pub enum Light {
    Red,
    Amber,
    Green,
}

pub fn wait(light: Light) -> u32 {
    match light {
        Light::Green => 0,
        Light::Red => 30,
        Light::Amber => 5,
    }
}

pub fn pause(light: Light) -> u32 {
    match light {
        Light::Red => 30,
        Light::Amber => 5,
        Light::Green => 0,
    }
}
