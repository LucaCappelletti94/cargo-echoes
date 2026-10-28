/// ```
/// use std::fmt::Debug;
/// struct Holder<'a>(&'a u8);
/// impl<'a> Holder<'a> {
///     fn show(&self) -> Box<dyn Debug + Send> {
///         Box::new(*self.0)
///     }
/// }
/// ```
pub struct Named;

/// ```
/// use std::fmt::Debug;
/// struct Holder<'a>(&'a u8);
/// impl Holder<'_> {
///     fn show(&self) -> Box<dyn Send + Debug> {
///         Box::new(*self.0)
///     }
/// }
/// ```
pub struct Anonymous;

/// ```
/// use std::fmt::Debug;
/// struct Holder<'a>(&'a u8);
/// impl<'a> Holder<'a> {
///     fn show(&self) -> Box<dyn Debug + Send + 'a> {
///         Box::new(self.0)
///     }
/// }
/// ```
pub struct Tied;

pub fn ordered(value: impl Clone + core::fmt::Debug) -> String {
    format!("{:?}", value.clone())
}

pub fn reversed(value: impl core::fmt::Debug + Clone) -> String {
    format!("{:?}", value.clone())
}
