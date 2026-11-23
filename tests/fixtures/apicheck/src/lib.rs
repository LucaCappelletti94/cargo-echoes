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
