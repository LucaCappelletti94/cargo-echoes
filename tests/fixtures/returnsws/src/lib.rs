/// ```
/// fn checked(limit: u32, value: u32) -> Result<u32, u8> {
///     if value > limit {
///         return Err(1);
///     }
///     return Ok(value);
/// }
/// ```
pub struct Returned;

/// ```
/// fn checked(limit: u32, value: u32) -> Result<u32, u8> {
///     if value > limit { Err(1) } else { Ok(value) }
/// }
/// ```
pub struct Branched;

/// ```
/// fn checked(limit: u32, value: u32) -> Result<u32, u8> {
///     if value > limit { Err(2) } else { Ok(value) }
/// }
/// ```
pub struct Coded;

pub fn bump(counter: &mut u32) -> Result<(), u8> {
    *counter += 1;
    return Ok(());
}

pub fn step(counter: &mut u32) -> Result<(), u8> {
    *counter += 1;
    Ok(())
}
