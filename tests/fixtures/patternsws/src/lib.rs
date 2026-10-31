/// ```
/// fn fields(line: &str) -> usize {
///     line.split(",").count()
/// }
/// assert_eq!(fields("a,b,c"), 3);
/// ```
pub struct Text;

/// ```
/// fn fields(line: &str) -> usize {
///     line.split(',').count()
/// }
/// assert_eq!(fields("a,b,c"), 3);
/// ```
pub struct Character;

/// ```
/// fn fields(line: &str) -> usize {
///     line.split(";").count()
/// }
/// assert_eq!(fields("a;b"), 2);
/// ```
pub struct Other;

pub fn newline(text: &mut String) {
    text.push_str("\n");
}

pub fn pushed(text: &mut String) {
    text.push('\n');
}

pub fn tabbed(text: &mut String) {
    text.push('\t');
}
