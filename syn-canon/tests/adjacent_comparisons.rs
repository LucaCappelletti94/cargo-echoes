//! Integer comparisons with a literal read as strict comparisons and unsigned zero tests.

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, read: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(read));
    assert_eq!(left, right, "{written}\n{read}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(read)),
        "{written}\n{read}"
    );
}

fn separate(written: &str, read: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(read)),
            "{written}\n{read}"
        );
    }
}

fn test(condition: &str) -> String {
    format!("fn f(x: u32, y: i32, z: u8) -> bool {{ {condition} }}")
}

#[test]
fn inclusive_bounds_read_as_strict_bounds() {
    for (written, read) in [
        ("x <= 5", "x < 6"),
        ("x >= 5", "x > 4"),
        ("5 >= x", "x < 6"),
        ("5 <= x", "4 < x"),
        ("y <= 5", "y < 6"),
        ("y >= -3i32", "y > -4i32"),
        ("y >= 0i32", "y > -1i32"),
        ("z <= 254", "z < 255"),
        ("!(x >= 5)", "x < 5"),
        ("!(x == 5)", "x != 5"),
        ("!(z <= 255)", "255 < z"),
    ] {
        equivalent(&test(written), &test(read));
    }
}

#[test]
fn unsigned_zero_bounds_read_as_zero_tests() {
    for (written, read) in [
        ("x >= 1", "x != 0"),
        ("x > 0", "x != 0"),
        ("0 < x", "0 != x"),
        ("x <= 0", "x == 0"),
        ("x < 1", "x == 0"),
        ("z >= 1u8", "z != 0u8"),
        ("!(x > 0)", "x == 0"),
    ] {
        equivalent(&test(written), &test(read));
    }
}

#[test]
fn branches_take_one_condition() {
    for (written, read) in [
        (
            "fn f(x: u32) -> u32 { if x >= 5 { 1 } else { 2 } }",
            "fn f(x: u32) -> u32 { if x < 5 { 2 } else { 1 } }",
        ),
        (
            "fn f(x: u32) -> u32 { if x <= 5 { 1 } else { 2 } }",
            "fn f(x: u32) -> u32 { if 5 < x { 2 } else { 1 } }",
        ),
        (
            "fn f(x: u32) -> u32 { if x > 0 { 1 } else { 2 } }",
            "fn f(x: u32) -> u32 { if x == 0 { 2 } else { 1 } }",
        ),
    ] {
        equivalent(written, read);
    }
}

#[test]
fn unproven_and_boundary_comparisons_keep_their_spelling() {
    for (written, read) in [
        ("y > 0", "y != 0"),
        ("y >= 1", "y != 0"),
        ("z <= 255", "z < 256"),
        ("y >= 0", "y > -1"),
        ("x >= 0", "x > 0"),
        ("y >= -2147483648i32", "y > -2147483649i32"),
    ] {
        separate(&test(written), &test(read));
    }
    separate(
        "fn f(p: bool, q: bool) -> bool { p <= q }",
        "fn f(p: bool, q: bool) -> bool { p < q }",
    );
    separate(
        "fn f(x: T) -> bool { x >= 1 }",
        "fn f(x: T) -> bool { x != 0 }",
    );
    separate(
        "fn f(x: f32) -> bool { x >= 1.0 }",
        "fn f(x: f32) -> bool { x > 0.0 }",
    );
}
