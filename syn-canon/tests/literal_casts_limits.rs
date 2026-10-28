//! Literal casts, primitive limits and unsigned power-of-two arithmetic.

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(written: &str, canonical: &str) {
    let left = syn_canon::canonicalize(parse(written));
    let right = syn_canon::canonicalize(parse(canonical));
    assert_eq!(left, right, "{written}\n{canonical}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{written}");
    assert_eq!(left.body_units(), right.body_units(), "{written}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(written)),
        syn_canon::canonicalize_failing(parse(canonical)),
        "{written}\n{canonical}"
    );
}

fn separate(written: &str, canonical: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(written)),
            canonicalize(parse(canonical)),
            "{written}\n{canonical}"
        );
    }
}

fn body(ty: &str, body: &str) -> String {
    format!("fn f(x: {ty}) -> {ty} {{ {body} }}")
}

fn test(ty: &str, body: &str) -> String {
    format!("fn f(x: {ty}) -> bool {{ {body} }}")
}

#[test]
fn literal_casts_and_limits_read_as_suffixed_literals() {
    for (written, read) in [
        ("fn f() -> u64 { 5 as u64 }", "fn f() -> u64 { 5u64 }"),
        ("fn f() -> i8 { 0x7f as i8 }", "fn f() -> i8 { 127i8 }"),
        ("fn f() -> u32 { b'a' as u32 }", "fn f() -> u32 { 97u32 }"),
        (
            "fn f() -> usize { 0 as usize }",
            "fn f() -> usize { 0usize }",
        ),
        (
            "fn f() -> isize { 32767 as isize }",
            "fn f() -> isize { 32767isize }",
        ),
        ("fn f() -> u8 { 'a' as u8 }", "fn f() -> u8 { 97u8 }"),
        ("fn f() -> u32 { '\\n' as u32 }", "fn f() -> u32 { 10u32 }"),
        ("fn f() -> u8 { b'a' }", "fn f() -> u8 { 97u8 }"),
        ("fn f() -> u8 { b'\\xff' }", "fn f() -> u8 { 255u8 }"),
        ("fn f() -> u8 { u8::MAX }", "fn f() -> u8 { 255u8 }"),
        ("fn f() -> u64 { u64::MIN }", "fn f() -> u64 { 0u64 }"),
        (
            "fn f() -> i32 { i32::MIN }",
            "fn f() -> i32 { -2147483648i32 }",
        ),
        ("fn f() -> i16 { i16::MAX }", "fn f() -> i16 { 32767i16 }"),
        (
            "type Byte = u8; fn f() -> Byte { Byte::MAX }",
            "type Byte = u8; fn f() -> Byte { 255u8 }",
        ),
        (
            "fn f(x: u8) -> u8 { x & u8::MAX }",
            "fn f(x: u8) -> u8 { x }",
        ),
    ] {
        equivalent(written, read);
    }
}

#[test]
fn unsigned_power_of_two_arithmetic_reads_as_bit_operations() {
    for (ty, written, read) in [
        ("u32", "x / 2", "x >> 1"),
        ("u64", "x / 8", "x >> 3"),
        ("u8", "x / 128u8", "x >> 7"),
        ("u32", "x % 8", "x & 7"),
        ("u16", "x % 2", "x & 1"),
        ("u32", "(x | 1) / 4", "(x | 1) >> 2"),
    ] {
        equivalent(&body(ty, written), &body(ty, read));
    }
}

#[test]
fn parity_tests_and_trivial_operations_read_alike() {
    for (ty, written, read) in [
        ("i32", "x % 2 == 0", "x & 1 == 0"),
        ("i64", "x % 2 != 0", "x & 1 != 0"),
        ("i8", "0 == x % 2", "x & 1 == 0"),
        ("u32", "x % 2 == 0", "x & 1 == 0"),
    ] {
        equivalent(&test(ty, written), &test(ty, read));
    }
    for (ty, written, read) in [
        ("u32", "x % 1", "0u32"),
        ("i32", "x % 1", "0i32"),
        ("u32", "!!x", "x"),
        ("i64", "!!x", "x"),
        ("u32", "x.pow(1)", "x"),
        ("i16", "x.pow(1)", "x"),
        ("bool", "!!x", "x"),
    ] {
        equivalent(&body(ty, written), &body(ty, read));
    }
}

#[test]
fn signed_division_wide_casts_and_shadowed_names_stay_apart() {
    for (written, read) in [
        (body("i32", "x / 2"), body("i32", "x >> 1")),
        (body("i32", "x % 4"), body("i32", "x & 3")),
        (body("u32", "x / 6"), body("u32", "x >> 2")),
        (test("i32", "x % 2 == 1"), test("i32", "x & 1 == 1")),
        (
            "fn f() -> usize { 70000 as usize }".to_owned(),
            "fn f() -> usize { 70000usize }".to_owned(),
        ),
        (
            "fn f() -> u8 { 'é' as u8 }".to_owned(),
            "fn f() -> u8 { 233u8 }".to_owned(),
        ),
        (
            "mod m { pub const MAX: u8 = 3; } use m as u8; fn f() -> core::primitive::u8 { u8::MAX }"
                .to_owned(),
            "mod m { pub const MAX: u8 = 3; } use m as u8; fn f() -> core::primitive::u8 { 255u8 }"
                .to_owned(),
        ),
        (
            "struct u64; fn f() -> core::primitive::u64 { 5 as u64 }".to_owned(),
            "struct u64; fn f() -> core::primitive::u64 { 5u64 }".to_owned(),
        ),
        (
            "fn f() -> usize { usize::MAX }".to_owned(),
            "fn f() -> usize { 18446744073709551615usize }".to_owned(),
        ),
        (body("u32", "x.pow(2)"), body("u32", "x")),
        (test("i32", "x % 3 == 0"), test("i32", "x & 3 == 0")),
        (test("i32", "x - 2 == 0"), test("i32", "x & 1 == 0")),
        (test("Custom", "x % 2 == 0"), test("Custom", "x & 1 == 0")),
        (
            "fn f() -> u8 { 1.5 as u8 }".to_owned(),
            "fn f() -> u8 { 1u8 }".to_owned(),
        ),
        (
            "fn f() -> u8 { 300 as u8 }".to_owned(),
            "fn f() -> u8 { 44u8 }".to_owned(),
        ),
        (
            "fn f() -> u8 { S::<u8>::MAX }".to_owned(),
            "fn f() -> u8 { 255u8 }".to_owned(),
        ),
        (
            "fn f() -> bool { bool::MAX }".to_owned(),
            "fn f() -> bool { true }".to_owned(),
        ),
        (
            "fn f() -> *const u8 { 0 as *const u8 }".to_owned(),
            "fn f() -> *const u8 { core::ptr::null() }".to_owned(),
        ),
        (
            "fn f() -> u32 { u32::BITS }".to_owned(),
            "fn f() -> u32 { 32u32 }".to_owned(),
        ),
        (
            "fn f(x: u8) -> u64 { x as u64 }".to_owned(),
            "fn f(x: u8) -> u64 { 5u64 }".to_owned(),
        ),
        (test("i32", "x % 2 == x"), test("i32", "x & 1 == x")),
    ] {
        separate(&written, &read);
    }
}
