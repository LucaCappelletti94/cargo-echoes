//! Scanned by the scheduled API check, whose branch appends a rewritten copy of `pack_argb`
//! and an indented copy of the unparseable doctest.

/// Packs four channels into one ARGB value, forcing full alpha when opaque and clearing the
/// colour of a fully transparent pixel.
fn pack_argb(red: u32, green: u32, blue: u32, alpha: u32, opaque: bool) -> u32 {
    let rgb = ((red & 0xffu32) << 16u32) | ((green & 0xffu32) << 8u32) | (blue & 0xffu32);
    let a = if opaque { 0xffu32 } else { alpha & 0xffu32 };
    let visible = if a == 0u32 { 0u32 } else { rgb };
    (a << 24u32) | visible
}

/// Unparseable body.
/// ```
/// @ nope nope
/// ```
pub fn unparsed() {}

/// The same packing, written again.
fn colour(x: u32, y: u32, z: u32, w: u32, solid: bool) -> u32 {
    // Blue first, then green and red, alpha on top.
    let unused = 17u32;
    let lo = 255u32 & z;
    let mid = (y & 255u32) << (2u32 * 4u32);
    let packed = lo | mid | (x & 0xffu32) << 16u32;
    let top = if !solid { w & 255u32 } else { 255u32 };
    let shown = if top == 0u32 { 0u32 } else { packed };
    return top << 24u32 | shown;
}

/// An indented copy added by the API check.
///
///     @ nope nope
pub fn unparsed_indented() {}
