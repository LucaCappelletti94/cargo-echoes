/// ```
/// enum Shape { Circle, Square, Line }
/// fn closed(shape: Shape) -> bool {
///     matches!(shape, Shape::Circle | Shape::Square)
/// }
/// assert!(closed(Shape::Square) && !closed(Shape::Line) && closed(Shape::Circle));
/// ```
pub struct Macro;

/// ```
/// enum Shape { Circle, Square, Line }
/// fn closed(shape: Shape) -> bool {
///     match shape {
///         Shape::Circle | Shape::Square => true,
///         _ => false,
///     }
/// }
/// assert!(closed(Shape::Square) && !closed(Shape::Line) && closed(Shape::Circle));
/// ```
pub struct Written;

/// ```
/// enum Shape { Circle, Square, Line }
/// fn closed(shape: Shape) -> bool {
///     matches!(shape, Shape::Circle | Shape::Line)
/// }
/// assert!(!closed(Shape::Square) && closed(Shape::Line) && closed(Shape::Circle));
/// ```
pub struct Other;

pub fn empty(value: Option<u32>) -> bool {
    !matches!(value, Some(_))
}

pub fn absent(value: Option<u32>) -> bool {
    match value {
        Some(_) => false,
        _ => true,
    }
}

pub fn present(value: Option<u32>) -> bool {
    matches!(value, Some(_))
}
