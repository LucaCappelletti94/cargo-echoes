//! A function view keeps the syntax an enclosing attribute macro reads, the derive input of a
//! std derive name its module may bind to another macro, and the format string of a call its
//! scopes do not prove to be the std formatting macro.

use syn_canon::{CanonicalForm, SourceContext};

/// The view of the function `f`, free or in an impl, canonicalized in its source context.
fn view(source: &str) -> CanonicalForm {
    let file: syn::File = syn::parse_str(source).unwrap_or_else(|e| panic!("{source}: {e}"));
    let context = SourceContext::new(
        core::iter::once((&[][..], &file)),
        syn_canon::Edition::E2021,
    );
    let (sig, block) = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(f) if f.sig.ident == "f" => Some((&f.sig, &*f.block)),
            syn::Item::Impl(i) => i.items.iter().find_map(|member| match member {
                syn::ImplItem::Fn(f) if f.sig.ident == "f" => Some((&f.sig, &f.block)),
                _ => None,
            }),
            _ => None,
        })
        .expect("a function f");
    context
        .function(sig, block)
        .expect("an indexed f")
        .canonicalize()
}

#[test]
fn an_enclosing_attribute_macro_reads_the_method_as_written() {
    let source = |attr: &str, body: &str| {
        format!("struct S;\n{attr}\nimpl S {{\n    fn f() -> u8 {{ {body} }}\n}}")
    };
    assert_ne!(
        view(&source("#[observe]", "return 1;")),
        view(&source("#[observe]", "1"))
    );
    assert_eq!(view(&source("", "return 1;")), view(&source("", "1")));
}

#[test]
fn a_module_binding_of_a_std_derive_name_keeps_the_derive_input() {
    let source = |scope: &str, derives: &str| {
        format!(
            "{scope}\nfn f() -> u8 {{\n    #[derive({derives})]\n    struct P {{ x: u8 }}\n    1\n}}"
        )
    };
    for scope in [
        "use count::Debug;",
        "use count::*;",
        "#[macro_use]\nextern crate count;",
    ] {
        assert_ne!(
            view(&source(scope, "Debug, Clone")),
            view(&source(scope, "Clone, Debug")),
            "{scope}"
        );
    }
    for scope in ["", "use std::fmt::Debug;", "use count::Other;"] {
        assert_eq!(
            view(&source(scope, "Debug, Clone")),
            view(&source(scope, "Clone, Debug")),
            "{scope}"
        );
    }
}

/// The view of the function `f` in module `m`, declared in its own file under a crate root
/// holding `root`.
fn module_view(root: &str, module: &str) -> CanonicalForm {
    let parse = |source: &str| -> syn::File {
        syn::parse_str(source).unwrap_or_else(|e| panic!("{source}: {e}"))
    };
    let (root, module) = (parse(&format!("mod m;\n{root}")), parse(module));
    let path = [String::from("m")];
    let context = SourceContext::new(
        [(&[][..], &root), (&path[..], &module)],
        syn_canon::Edition::E2021,
    );
    let f = module
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(f) if f.sig.ident == "f" => Some(f),
            _ => None,
        })
        .expect("a function f");
    context
        .function(&f.sig, &f.block)
        .expect("an indexed f")
        .canonicalize()
}

#[test]
fn format_captures_rename_when_the_module_proves_the_std_macro() {
    let capture = |module: &str, name: &str| {
        module_view(
            "",
            &format!("{module}\nfn f({name}: u8) -> String {{ format!(\"{{{name}}}\") }}"),
        )
    };
    for module in [
        "#[derive(Debug)]\nstruct S;\n#[cfg(test)]\nmod tests {}",
        "use std::fmt::*;",
        "use super::*;",
    ] {
        assert_eq!(capture(module, "x"), capture(module, "y"), "{module}");
    }
    for module in ["use count::format;", "use count::*;"] {
        assert_ne!(capture(module, "x"), capture(module, "y"), "{module}");
    }
}

#[test]
fn a_super_glob_reads_the_formatting_macros_of_the_parent_module() {
    let capture = |root: &str, name: &str| {
        module_view(
            root,
            &format!("use super::*;\nfn f({name}: u8) -> String {{ format!(\"{{{name}}}\") }}"),
        )
    };
    for root in ["use count::format;", "use count::*;"] {
        assert_ne!(capture(root, "x"), capture(root, "y"), "{root}");
    }
    assert_eq!(
        capture("use count::Other;", "x"),
        capture("use count::Other;", "y")
    );
    let past_root = |name: &str| {
        module_view(
            "",
            &format!(
                "use super::super::*;\nfn f({name}: u8) -> String {{ format!(\"{{{name}}}\") }}"
            ),
        )
    };
    assert_ne!(past_root("x"), past_root("y"));
}

#[test]
fn reflexive_module_views_respect_primitive_shadowing() {
    let body = |expression: &str| format!("fn f(x: u32) -> bool {{ {expression} }}");
    assert_eq!(
        module_view("", &body("x == x")),
        module_view("", &body("true")),
    );
    let root = "#[derive(PartialEq)] pub struct Token;";
    let shadowed = |expression: &str| format!("use super::Token as u32; {}", body(expression));
    assert_ne!(
        module_view(root, &shadowed("x == x")),
        module_view(root, &shadowed("true")),
    );
}

/// The canonical form of the whole file under the compilation-failure mode.
fn failing_form(source: &str) -> CanonicalForm {
    syn_canon::canonicalize_failing(
        syn::parse_str(source).unwrap_or_else(|error| panic!("{source}: {error}")),
    )
}

#[test]
fn proven_reflexive_equalities_fold_in_the_borrowed_view() {
    assert_eq!(
        view("fn f(x: u32) -> bool { x == x }"),
        view("fn f(x: u32) -> bool { true }")
    );
    assert_eq!(
        view("fn f(flag: bool) -> bool { flag == flag }"),
        view("fn f(flag: bool) -> bool { true }")
    );
    assert_eq!(
        view("fn f(x: u32) -> bool { (x & 7u32) == (x & 7u32) }"),
        view("fn f(x: u32) -> bool { true }")
    );
    assert_eq!(
        view("fn h(x: u32) -> u32 { x & 7u32 }\nfn f(x: u32) -> bool { h(x) == h(x) }"),
        view("fn h(x: u32) -> u32 { x & 7u32 }\nfn f(x: u32) -> bool { true }")
    );
    assert_eq!(
        view("fn f(x: u32) -> bool { read(); x == x }"),
        view("fn f(x: u32) -> bool { read(); true }")
    );
}

#[test]
fn the_failure_mode_retains_the_reflexive_source_distinction() {
    assert_ne!(
        failing_form("fn f(x: u32) -> bool { x == x }"),
        failing_form("fn f(x: u32) -> bool { true }")
    );
}

#[test]
fn an_observing_impl_retains_the_reflexive_comparison() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f(x: u32) -> bool {{ {body} }}\n}}")
    };
    assert_ne!(view(&source("x == x")), view(&source("true")));
}

#[test]
fn proven_xor_cancellations_fold_in_the_borrowed_view() {
    assert_eq!(
        view("fn f(x: u32) -> u32 { x ^ x }"),
        view("fn f(x: u32) -> u32 { 0u32 }")
    );
    assert_eq!(
        view("fn f(p: bool) -> bool { p ^ p }"),
        view("fn f(p: bool) -> bool { false }")
    );
    assert_eq!(
        view("fn f(x: u32, y: u32) -> u32 { x ^ y ^ x }"),
        view("fn f(x: u32, y: u32) -> u32 { y }")
    );
    assert_eq!(
        view("fn f(x: u32) -> u32 { x ^ x ^ x }"),
        view("fn f(x: u32) -> u32 { x }")
    );
    assert_eq!(
        view("fn f(x: u32) -> u32 { x ^ 0u32 }"),
        view("fn f(x: u32) -> u32 { x }")
    );
    assert_eq!(
        view("fn f(x: u32) -> u32 { read(); x ^ x }"),
        view("fn f(x: u32) -> u32 { read(); 0u32 }")
    );
    assert_eq!(
        view("fn h(x: u32) -> u32 { x ^ 7u32 }\nfn f(x: u32) -> u32 { h(x) ^ h(x) }"),
        view("fn h(x: u32) -> u32 { x ^ 7u32 }\nfn f(x: u32) -> u32 { 0u32 }")
    );
}

#[test]
fn unproven_xor_operands_do_not_cancel_in_the_borrowed_view() {
    assert_ne!(
        view("fn f() -> u32 { read() ^ read() }"),
        view("fn f() -> u32 { 0u32 }")
    );
    assert_ne!(
        view("fn f(x: u32) -> u32 { (1u32 / x) ^ (1u32 / x) }"),
        view("fn f(x: u32) -> u32 { 0u32 }")
    );
    assert_ne!(
        view("fn f(x: u32, y: u32) -> u32 { x ^ y ^ x }"),
        view("fn f(x: u32, y: u32) -> u32 { 0u32 }")
    );
    let custom = "#[derive(Clone, Copy)] struct T;\nimpl core::ops::BitXor for T {\n    type Output = T;\n    fn bitxor(self, _other: T) -> T { T }\n}\n";
    assert_ne!(
        view(&format!("{custom}fn f(x: T) -> T {{ x ^ x }}")),
        view(&format!("{custom}fn f(x: T) -> T {{ T }}"))
    );
}

#[test]
fn the_failure_mode_retains_the_xor_source_distinction() {
    assert_ne!(
        failing_form("fn f(x: u32) -> u32 { x ^ x }"),
        failing_form("fn f(x: u32) -> u32 { 0u32 }")
    );
}

#[test]
fn an_observing_impl_retains_the_xor_cancellation() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f(x: u32) -> u32 {{ {body} }}\n}}")
    };
    assert_ne!(view(&source("x ^ x")), view(&source("0u32")));
}

#[test]
fn proven_wrapping_add_operands_share_one_order_in_the_borrowed_view() {
    let body = |expression: &str| format!("fn f(x: u32, y: u32) -> u32 {{ {expression} }}");
    for (written, reordered) in [
        ("x.wrapping_add(y)", "y.wrapping_add(x)"),
        ("x.wrapping_add(7u32)", "7u32.wrapping_add(x)"),
        ("read(); x.wrapping_add(y)", "read(); y.wrapping_add(x)"),
    ] {
        assert_eq!(view(&body(written)), view(&body(reordered)), "{written}");
    }
    assert_eq!(
        view(
            "fn h(x: u32) -> u32 { x & 7u32 }\nfn f(x: u32, y: u32) -> u32 { h(x).wrapping_add(y) }"
        ),
        view(
            "fn h(x: u32) -> u32 { x & 7u32 }\nfn f(x: u32, y: u32) -> u32 { y.wrapping_add(h(x)) }"
        )
    );
}

#[test]
fn unproven_wrapping_add_operands_keep_their_order_in_the_borrowed_view() {
    assert_ne!(
        view("fn f(x: u32) -> u32 { read().wrapping_add(x) }"),
        view("fn f(x: u32) -> u32 { x.wrapping_add(read()) }")
    );
    let custom = "#[derive(Clone, Copy)] struct T(u32);\nimpl T {\n    fn wrapping_add(self, other: T) -> T { T(self.0.wrapping_sub(other.0)) }\n}\n";
    assert_ne!(
        view(&format!(
            "{custom}fn f(x: T, y: T) -> T {{ x.wrapping_add(y) }}"
        )),
        view(&format!(
            "{custom}fn f(x: T, y: T) -> T {{ y.wrapping_add(x) }}"
        ))
    );
}

#[test]
fn wrapping_add_module_views_respect_primitive_shadowing() {
    let body = |expression: &str| format!("fn f(x: u32, y: u32) -> u32 {{ {expression} }}");
    assert_eq!(
        module_view("", &body("x.wrapping_add(y)")),
        module_view("", &body("y.wrapping_add(x)")),
    );
    let root = "#[derive(Clone, Copy)] pub struct Token;\nimpl Token { pub fn wrapping_add(self, _: Token) -> Token { Token } }";
    let shadowed = |expression: &str| format!("use super::Token as u32; {}", body(expression));
    assert_ne!(
        module_view(root, &shadowed("x.wrapping_add(y)")),
        module_view(root, &shadowed("y.wrapping_add(x)")),
    );
}

#[test]
fn the_failure_mode_retains_the_wrapping_add_operand_order() {
    assert_ne!(
        failing_form("fn f(x: u32, y: u32) -> u32 { x.wrapping_add(y) }"),
        failing_form("fn f(x: u32, y: u32) -> u32 { y.wrapping_add(x) }")
    );
}

#[test]
fn an_observing_impl_retains_the_wrapping_add_operand_order() {
    let source = |body: &str| {
        format!(
            "struct S;\n#[observe]\nimpl S {{\n    fn f(x: u32, y: u32) -> u32 {{ {body} }}\n}}"
        )
    };
    assert_ne!(
        view(&source("x.wrapping_add(y)")),
        view(&source("y.wrapping_add(x)"))
    );
}

#[test]
fn proven_boolean_matches_fold_into_the_conditional_in_the_borrowed_view() {
    let body =
        |expression: &str| format!("fn f(p: bool, x: u32, y: u32) -> u32 {{ {expression} }}");
    for (matched, conditional) in [
        ("match p { true => x, false => y }", "if p { x } else { y }"),
        ("match p { false => y, true => x }", "if p { x } else { y }"),
        (
            "read(); match p { false => y, true => x }",
            "read(); if p { x } else { y }",
        ),
    ] {
        assert_eq!(view(&body(matched)), view(&body(conditional)), "{matched}");
    }
}

#[test]
fn unproven_boolean_matches_stay_matches_in_the_borrowed_view() {
    assert_ne!(
        view("fn f(x: u32, y: u32) -> u32 { match read() { true => x, false => y } }"),
        view("fn f(x: u32, y: u32) -> u32 { if read() { x } else { y } }")
    );
    assert_ne!(
        view("fn f(p: bool, y: u32) -> u32 { match p { true => read(), false => y } }"),
        view("fn f(p: bool, y: u32) -> u32 { if p { read() } else { y } }")
    );
}

#[test]
fn the_failure_mode_retains_the_boolean_match() {
    assert_ne!(
        failing_form("fn f(p: bool, x: u32, y: u32) -> u32 { match p { true => x, false => y } }"),
        failing_form("fn f(p: bool, x: u32, y: u32) -> u32 { if p { x } else { y } }")
    );
}

#[test]
fn an_observing_impl_retains_the_boolean_match() {
    let source = |body: &str| {
        format!(
            "struct S;\n#[observe]\nimpl S {{\n    fn f(p: bool, x: u32, y: u32) -> u32 {{ {body} }}\n}}"
        )
    };
    assert_ne!(
        view(&source("match p { true => x, false => y }")),
        view(&source("if p { x } else { y }"))
    );
}

#[test]
fn proven_literal_comparisons_fold_in_the_borrowed_view() {
    let body = |expression: &str| format!("fn f(p: bool) -> bool {{ {expression} }}");
    for (written, folded) in [
        ("p == false", "!p"),
        ("true != p", "!p"),
        ("read(); p != false", "read(); p"),
    ] {
        assert_eq!(view(&body(written)), view(&body(folded)), "{written}");
    }
}

#[test]
fn unproven_literal_comparisons_stay_comparisons_in_the_borrowed_view() {
    assert_ne!(
        view("fn f(z: Unknown) -> bool { z == false }"),
        view("fn f(z: Unknown) -> bool { !z }")
    );
    assert_ne!(
        view("fn f() -> bool { read() != true }"),
        view("fn f() -> bool { !read() }")
    );
}

#[test]
fn the_failure_mode_retains_the_literal_comparison() {
    assert_ne!(
        failing_form("fn f(p: bool) -> bool { p == false }"),
        failing_form("fn f(p: bool) -> bool { !p }")
    );
}

#[test]
fn an_observing_impl_retains_the_literal_comparison() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f(p: bool) -> bool {{ {body} }}\n}}")
    };
    assert_ne!(view(&source("p == false")), view(&source("!p")));
}

#[test]
fn proven_logical_chains_take_one_form_in_the_borrowed_view() {
    let body = |expression: &str| format!("fn f(p: bool, q: bool) -> bool {{ {expression} }}");
    for (written, folded) in [
        ("q && p", "p && q"),
        ("if p { q } else { false }", "p && q"),
        ("p || false || p", "p"),
        ("read(); q || p", "read(); p || q"),
    ] {
        assert_eq!(view(&body(written)), view(&body(folded)), "{written}");
    }
}

#[test]
fn unproven_logical_operands_keep_their_order_in_the_borrowed_view() {
    assert_ne!(
        view("fn f(p: bool, z: Unknown) -> bool { z && p }"),
        view("fn f(p: bool, z: Unknown) -> bool { p && z }")
    );
    assert_ne!(
        view("fn f(p: bool) -> bool { read() && p }"),
        view("fn f(p: bool) -> bool { p && read() }")
    );
}

#[test]
fn the_failure_mode_retains_the_logical_operand_order() {
    assert_ne!(
        failing_form("fn f(p: bool, q: bool) -> bool { q && p }"),
        failing_form("fn f(p: bool, q: bool) -> bool { p && q }")
    );
}

#[test]
fn an_observing_impl_retains_the_logical_operand_order() {
    let source = |body: &str| {
        format!(
            "struct S;\n#[observe]\nimpl S {{\n    fn f(p: bool, q: bool) -> bool {{ {body} }}\n}}"
        )
    };
    assert_ne!(view(&source("q && p")), view(&source("p && q")));
}

#[test]
fn proven_compound_assignments_expand_in_the_borrowed_view() {
    let body = |statements: &str| format!("fn f(mut x: u32, y: u32) -> u32 {{ {statements} x }}");
    for (written, expanded) in [
        ("x += y;", "x = x + y;"),
        ("x <<= 3; x |= y;", "x = x << 3; x = x | y;"),
        ("read(); x -= y;", "read(); x = x - y;"),
    ] {
        assert_eq!(view(&body(written)), view(&body(expanded)), "{written}");
    }
}

#[test]
fn unproven_compound_targets_stay_compound_in_the_borrowed_view() {
    assert_ne!(
        view("fn f(mut x: Unknown, y: Unknown) -> Unknown { x += y; x }"),
        view("fn f(mut x: Unknown, y: Unknown) -> Unknown { x = x + y; x }")
    );
    assert_ne!(
        view("fn f(x: &mut u32, y: u32) { *x += y; }"),
        view("fn f(x: &mut u32, y: u32) { *x = *x + y; }")
    );
}

#[test]
fn the_failure_mode_retains_the_compound_assignment() {
    assert_ne!(
        failing_form("fn f(mut x: u32, y: u32) -> u32 { x += y; x }"),
        failing_form("fn f(mut x: u32, y: u32) -> u32 { x = x + y; x }")
    );
}

#[test]
fn an_observing_impl_retains_the_compound_assignment() {
    let source = |body: &str| {
        format!(
            "struct S;\n#[observe]\nimpl S {{\n    fn f(mut x: u32, y: u32) -> u32 {{ {body} x }}\n}}"
        )
    };
    assert_ne!(view(&source("x += y;")), view(&source("x = x + y;")));
}

#[test]
fn stated_let_annotations_drop_in_the_borrowed_view() {
    let body = |statements: &str| format!("fn f(x: u32) -> u32 {{ {statements} }}");
    for (annotated, suffixed) in [
        ("let v: u32 = 5; v ^ x", "let v = 5u32; v ^ x"),
        (
            "let mut v: u32 = 0x10; v += x; v",
            "let mut v = 16u32; v += x; v",
        ),
        ("read(); let v: u32 = 1; v", "read(); let v = 1u32; v"),
    ] {
        assert_eq!(view(&body(annotated)), view(&body(suffixed)), "{annotated}");
    }
}

#[test]
fn unproven_let_annotations_stay_in_the_borrowed_view() {
    assert_ne!(
        view("fn f() -> usize { let v: usize = 5; v }"),
        view("fn f() -> usize { let v = 5usize; v }")
    );
    assert_ne!(
        view("fn f(x: u32) -> u32 { let v: u32 = g(x); v }"),
        view("fn f(x: u32) -> u32 { let v = g(x); v }")
    );
}

#[test]
fn the_failure_mode_retains_the_let_annotation() {
    assert_ne!(
        failing_form("fn f() -> u32 { let v: u32 = 5; v }"),
        failing_form("fn f() -> u32 { let v = 5u32; v }")
    );
}

#[test]
fn an_observing_impl_retains_the_let_annotation() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f() -> u32 {{ {body} }}\n}}")
    };
    assert_ne!(
        view(&source("let v: u32 = 5; v")),
        view(&source("let v = 5u32; v"))
    );
}

#[test]
fn proven_commutative_operands_take_one_order_in_the_borrowed_view() {
    let body = |expression: &str| format!("fn f(x: u32, y: u32) -> u32 {{ {expression} }}");
    for (written, reordered) in [
        ("x + y", "y + x"),
        ("x.saturating_mul(y)", "y.saturating_mul(x)"),
        ("read(); x * y", "read(); y * x"),
    ] {
        assert_eq!(view(&body(written)), view(&body(reordered)), "{written}");
    }
}

#[test]
fn unproven_commutative_operands_keep_their_order_in_the_borrowed_view() {
    assert_ne!(
        view("fn f(x: u32, y: Unknown) -> u32 { (x + y) ^ x }"),
        view("fn f(x: u32, y: Unknown) -> u32 { (y + x) ^ x }")
    );
    assert_ne!(
        view("fn f(x: u32) -> u32 { read() + x }"),
        view("fn f(x: u32) -> u32 { x + read() }")
    );
}

#[test]
fn the_failure_mode_retains_the_commutative_operand_order() {
    assert_ne!(
        failing_form("fn f(x: u32, y: u32) -> u32 { y + x }"),
        failing_form("fn f(x: u32, y: u32) -> u32 { x + y }")
    );
}

#[test]
fn an_observing_impl_retains_the_commutative_operand_order() {
    let source = |body: &str| {
        format!(
            "struct S;\n#[observe]\nimpl S {{\n    fn f(x: u32, y: u32) -> u32 {{ {body} }}\n}}"
        )
    };
    assert_ne!(view(&source("y * x")), view(&source("x * y")));
}

#[test]
fn proven_identities_reduce_in_the_borrowed_view() {
    let body = |expression: &str| format!("fn f(x: u32, y: u32) -> u32 {{ {expression} }}");
    for (written, reduced) in [
        ("(x + 0) ^ y", "x ^ y"),
        ("x.wrapping_mul(1) | (y & 0)", "x"),
        ("read(); x - x", "read(); 0u32"),
    ] {
        assert_eq!(view(&body(written)), view(&body(reduced)), "{written}");
    }
}

#[test]
fn unproven_identities_stay_in_the_borrowed_view() {
    assert_ne!(
        view("fn f(x: Unknown) -> Unknown { x + 0 }"),
        view("fn f(x: Unknown) -> Unknown { x }")
    );
    assert_ne!(
        view("fn f() -> u32 { read() * 0 }"),
        view("fn f() -> u32 { 0u32 }")
    );
}

#[test]
fn the_failure_mode_retains_the_identity_operand() {
    assert_ne!(
        failing_form("fn f(x: u32) -> u32 { x | 0 }"),
        failing_form("fn f(x: u32) -> u32 { x }")
    );
}

#[test]
fn an_observing_impl_retains_the_identity_operand() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f(x: u32) -> u32 {{ {body} }}\n}}")
    };
    assert_ne!(view(&source("x * 1")), view(&source("x")));
}

#[test]
fn nested_conditionals_merge_in_the_borrowed_view() {
    let body =
        |statements: &str| format!("fn f(p: bool, q: bool, x: u32) -> u32 {{ {statements} }}");
    for (written, merged) in [
        ("if p { if q { mark(); } } x", "if p && q { mark(); } x"),
        (
            "if p { if q { x } else { read() } } else { read() }",
            "if q && p { x } else { read() }",
        ),
        (
            "match p { false => 0u32, _ => x }",
            "if p { x } else { 0u32 }",
        ),
    ] {
        assert_eq!(view(&body(written)), view(&body(merged)), "{written}");
    }
}

#[test]
fn unproven_nested_conditionals_stay_nested_in_the_borrowed_view() {
    assert_ne!(
        view("fn f(p: Unknown, q: bool) { if p { if q { mark(); } } }"),
        view("fn f(p: Unknown, q: bool) { if p && q { mark(); } }")
    );
    assert_ne!(
        view(
            "fn f(p: bool, q: bool, x: u32) -> u32 { if p { if q { x } else { 1u32 } } else { 2u32 } }"
        ),
        view("fn f(p: bool, q: bool, x: u32) -> u32 { if p && q { x } else { 2u32 } }")
    );
}

#[test]
fn the_failure_mode_retains_the_nested_conditional() {
    assert_ne!(
        failing_form("fn f(p: bool, q: bool) { if p { if q { mark(); } } }"),
        failing_form("fn f(p: bool, q: bool) { if p && q { mark(); } }")
    );
}

#[test]
fn an_observing_impl_retains_the_nested_conditional() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f(p: bool, q: bool) {{ {body} }}\n}}")
    };
    assert_ne!(
        view(&source("if p { if q { mark(); } }")),
        view(&source("if p && q { mark(); }"))
    );
}

#[test]
fn early_returns_read_as_else_branches_in_the_borrowed_view() {
    let body =
        |statements: &str| format!("fn f(p: bool, q: bool, x: u32) -> u32 {{ {statements} }}");
    for (written, branched) in [
        (
            "if p { mark(); return x; } 0u32",
            "if p { mark(); x } else { 0u32 }",
        ),
        (
            "if p { return x; } if q { return 1u32; } 0u32",
            "if p { x } else if q { 1u32 } else { 0u32 }",
        ),
    ] {
        assert_eq!(view(&body(written)), view(&body(branched)), "{written}");
    }
}

#[test]
fn unproven_early_returns_keep_the_return_in_the_borrowed_view() {
    assert_ne!(
        view("fn f(p: Unknown, x: u32) -> u32 { if p { return x; } 0u32 }"),
        view("fn f(p: Unknown, x: u32) -> u32 { if p { x } else { 0u32 } }")
    );
    assert_ne!(
        view("fn f(p: bool, x: u32) -> u32 { if p { return x; } read() }"),
        view("fn f(p: bool, x: u32) -> u32 { if p { x } else { read() } }")
    );
}

#[test]
fn the_failure_mode_retains_the_early_return() {
    assert_ne!(
        failing_form("fn f(p: bool, x: u32) -> u32 { if p { return x; } 0u32 }"),
        failing_form("fn f(p: bool, x: u32) -> u32 { if p { x } else { 0u32 } }")
    );
}

#[test]
fn an_observing_impl_retains_the_early_return() {
    let source = |body: &str| {
        format!(
            "struct S;\n#[observe]\nimpl S {{\n    fn f(p: bool, x: u32) -> u32 {{ {body} }}\n}}"
        )
    };
    assert_ne!(
        view(&source("if p { return x; } 0u32")),
        view(&source("if p { x } else { 0u32 }"))
    );
}

#[test]
fn split_arms_and_let_else_read_as_their_joined_forms_in_the_borrowed_view() {
    for (written, joined) in [
        (
            "fn f(x: u32, y: u32) -> u32 { match x { 0 => y, 1 => y, _ => x } }",
            "fn f(x: u32, y: u32) -> u32 { match x { 0 | 1 => y, _ => x } }",
        ),
        (
            "fn f(o: Option<u32>) -> u32 { let Some(v) = o else { return 0; }; mark(v); v }",
            "fn f(o: Option<u32>) -> u32 { match o { Some(v) => { mark(v); v } _ => { return 0; } } }",
        ),
    ] {
        assert_eq!(view(written), view(joined), "{written}");
    }
}

#[test]
fn bound_arms_and_temporary_scrutinees_stay_apart_in_the_borrowed_view() {
    assert_ne!(
        view("fn f(t: (u32, u32)) -> u32 { match t { (a, 0) => a, (a, 1) => a, _ => 0 } }"),
        view("fn f(t: (u32, u32)) -> u32 { match t { (a, 0) | (a, 1) => a, _ => 0 } }")
    );
    assert_ne!(
        view("fn f() -> u32 { let Some(v) = make() else { return 0; }; v }"),
        view("fn f() -> u32 { match make() { Some(v) => { v } _ => { return 0; } } }")
    );
}

#[test]
fn the_failure_mode_retains_split_arms_and_let_else() {
    assert_ne!(
        failing_form("fn f(x: u32, y: u32) -> u32 { match x { 0 => y, 1 => y, _ => x } }"),
        failing_form("fn f(x: u32, y: u32) -> u32 { match x { 0 | 1 => y, _ => x } }")
    );
    assert_ne!(
        failing_form("fn f(o: Option<u32>) -> u32 { let Some(v) = o else { return 0; }; v }"),
        failing_form(
            "fn f(o: Option<u32>) -> u32 { match o { Some(v) => { v } _ => { return 0; } } }"
        )
    );
}

#[test]
fn an_observing_impl_retains_split_arms_and_let_else() {
    let source = |body: &str| {
        format!(
            "struct S;\n#[observe]\nimpl S {{\n    fn f(o: Option<u32>, x: u32) -> u32 {{ {body} }}\n}}"
        )
    };
    assert_ne!(
        view(&source("match x { 0 => x, 1 => x, _ => 2u32 }")),
        view(&source("match x { 0 | 1 => x, _ => 2u32 }"))
    );
    assert_ne!(
        view(&source("let Some(v) = o else { return 0; }; v")),
        view(&source("match o { Some(v) => { v } _ => { return 0; } } "))
    );
}

#[test]
fn owned_strings_and_empty_vectors_match_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "fn f() -> String { \"a\".to_owned() }",
            "fn f() -> String { String::from(\"a\") }",
        ),
        (
            "fn f() -> Vec<u32> { vec![] }",
            "fn f() -> Vec<u32> { Vec::new() }",
        ),
    ] {
        assert_eq!(view(written), view(canonical), "{written}");
    }
}

#[test]
fn claimed_prelude_names_keep_the_spelling_in_the_borrowed_view() {
    for prefix in ["#![no_std]", "use custom::ToOwned;", "use other::*;"] {
        assert_ne!(
            view(&format!("{prefix} fn f() -> String {{ \"a\".to_owned() }}")),
            view(&format!(
                "{prefix} fn f() -> String {{ \"a\".to_string() }}"
            ))
        );
    }
    assert_ne!(
        view("struct Vec; impl Vec { fn new() -> Self { Vec } } fn f() -> Vec { Vec::new() }"),
        view("struct Vec; impl Vec { fn new() -> Self { Vec } } fn f() -> Vec { vec![] }")
    );
}

#[test]
fn the_failure_mode_retains_owned_string_spellings() {
    assert_ne!(
        failing_form("fn f() -> String { \"a\".to_owned() }"),
        failing_form("fn f() -> String { \"a\".to_string() }")
    );
    assert_ne!(
        failing_form("fn f() -> Vec<u32> { vec![] }"),
        failing_form("fn f() -> Vec<u32> { Vec::new() }")
    );
}

#[test]
fn an_observing_impl_retains_owned_string_spellings() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f() -> String {{ {body} }}\n}}")
    };
    assert_ne!(
        view(&source("\"a\".to_owned()")),
        view(&source("\"a\".to_string()"))
    );
}

#[test]
fn widening_casts_match_conversions_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "fn f(x: u32) -> u64 { x as u64 }",
            "fn f(x: u32) -> u64 { u64::from(x) }",
        ),
        (
            "fn f(x: u32) -> u32 { x as u32 }",
            "fn f(x: u32) -> u32 { x }",
        ),
    ] {
        assert_eq!(view(written), view(canonical), "{written}");
    }
}

#[test]
fn lossy_and_shadowed_conversions_stay_apart_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "fn f(x: u32) -> u16 { x as u16 }",
            "fn f(x: u32) -> u16 { u16::from(x) }",
        ),
        (
            "use custom::u64; fn f(x: u32) -> u64 { x as u64 }",
            "use custom::u64; fn f(x: u32) -> u64 { u64::from(x) }",
        ),
        (
            "#![no_std] fn f(x: u32) -> u64 { x as u64 }",
            "#![no_std] fn f(x: u32) -> u64 { u64::from(x) }",
        ),
    ] {
        assert_ne!(view(written), view(canonical), "{written}");
    }
}

#[test]
fn the_failure_mode_retains_cast_spellings() {
    assert_ne!(
        failing_form("fn f(x: u32) -> u64 { x as u64 }"),
        failing_form("fn f(x: u32) -> u64 { u64::from(x) }")
    );
    assert_ne!(
        failing_form("fn f(x: u32) -> u32 { x as u32 }"),
        failing_form("fn f(x: u32) -> u32 { x }")
    );
}

#[test]
fn an_observing_impl_retains_cast_spellings() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f(x: u32) -> u64 {{ {body} }}\n}}")
    };
    assert_ne!(view(&source("x as u64")), view(&source("u64::from(x)")));
}

#[test]
fn bound_order_and_elidable_lifetimes_match_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "fn f<T: Copy + Clone>(t: T) -> T { t }",
            "fn f<T>(t: T) -> T where T: Clone + Copy { t }",
        ),
        (
            "fn f<'a>(x: &'a u32) -> &'a u32 { x }",
            "fn f(x: &u32) -> &u32 { x }",
        ),
    ] {
        assert_eq!(view(written), view(canonical), "{written}");
    }
}

#[test]
fn shared_and_function_bound_signatures_stay_apart_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "fn f<'a>(x: &'a u8, y: &'a u8) -> &'a u8 { x }",
            "fn f(x: &u8, y: &u8) -> &u8 { x }",
        ),
        (
            "fn f<F: Fn() + Clone>(g: F) {}",
            "fn f<F: Clone + Fn()>(g: F) {}",
        ),
    ] {
        assert_ne!(view(written), view(canonical), "{written}");
    }
}

#[test]
fn the_failure_mode_retains_bound_order_and_lifetimes() {
    assert_ne!(
        failing_form("fn f<T: Copy + Clone>(t: T) -> T { t }"),
        failing_form("fn f<T: Clone + Copy>(t: T) -> T { t }")
    );
    assert_ne!(
        failing_form("fn f<'a>(x: &'a u32) -> &'a u32 { x }"),
        failing_form("fn f(x: &u32) -> &u32 { x }")
    );
}

#[test]
fn an_observing_impl_retains_bound_order_and_lifetimes() {
    let source = |method: &str| format!("struct S;\n#[observe]\nimpl S {{\n    {method}\n}}");
    assert_ne!(
        view(&source("fn f<T: Copy + Clone>(t: T) -> T { t }")),
        view(&source("fn f<T: Clone + Copy>(t: T) -> T { t }"))
    );
    assert_ne!(
        view(&source("fn f<'a>(x: &'a u32) -> &'a u32 { x }")),
        view(&source("fn f(x: &u32) -> &u32 { x }"))
    );
}

#[test]
fn primitive_copies_reduce_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "fn f(x: u32) -> u32 { x.clone() }",
            "fn f(x: u32) -> u32 { x }",
        ),
        ("fn f(x: u32) -> u32 { *&x }", "fn f(x: u32) -> u32 { x }"),
        (
            "fn f(x: u32, y: u32) -> bool { &x == &y }",
            "fn f(x: u32, y: u32) -> bool { x == y }",
        ),
    ] {
        assert_eq!(view(written), view(canonical), "{written}");
    }
}

#[test]
fn foreign_imports_and_values_keep_their_copies_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "use other::Dup; fn f(x: u32) -> u32 { x.clone() }",
            "use other::Dup; fn f(x: u32) -> u32 { x }",
        ),
        (
            "fn f(s: String) -> String { s.clone() }",
            "fn f(s: String) -> String { s }",
        ),
    ] {
        assert_ne!(view(written), view(canonical), "{written}");
    }
}

#[test]
fn the_failure_mode_retains_primitive_copies() {
    assert_ne!(
        failing_form("fn f(x: u32) -> u32 { x.clone() }"),
        failing_form("fn f(x: u32) -> u32 { x }")
    );
    assert_ne!(
        failing_form("fn f(x: u32) -> u32 { *&x }"),
        failing_form("fn f(x: u32) -> u32 { x }")
    );
}

#[test]
fn an_observing_impl_retains_primitive_copies() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f(x: u32) -> u32 {{ {body} }}\n}}")
    };
    assert_ne!(view(&source("x.clone()")), view(&source("x")));
    assert_ne!(view(&source("*&x")), view(&source("x")));
}

#[test]
fn constructor_returns_read_as_tails_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "fn f() -> Result<(), u8> { mark(); return Ok(()); }",
            "fn f() -> Result<(), u8> { mark(); Ok(()) }",
        ),
        (
            "fn f(p: bool, x: u32) -> Option<u32> { if p { return None; } Some(x) }",
            "fn f(p: bool, x: u32) -> Option<u32> { if p { None } else { Some(x) } }",
        ),
    ] {
        assert_eq!(view(written), view(canonical), "{written}");
    }
}

#[test]
fn claimed_constructors_and_temporaries_keep_their_returns_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "use custom::Ok; fn f() -> Result<(), u8> { return Ok(()); }",
            "use custom::Ok; fn f() -> Result<(), u8> { Ok(()) }",
        ),
        (
            "fn f() -> Option<usize> { return Some(make().len()); }",
            "fn f() -> Option<usize> { Some(make().len()) }",
        ),
    ] {
        assert_ne!(view(written), view(canonical), "{written}");
    }
}

#[test]
fn the_failure_mode_retains_constructor_returns() {
    assert_ne!(
        failing_form("fn f() -> Result<(), u8> { return Ok(()); }"),
        failing_form("fn f() -> Result<(), u8> { Ok(()) }")
    );
}

#[test]
fn an_observing_impl_retains_constructor_returns() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f() -> Result<(), u8> {{ {body} }}\n}}")
    };
    assert_ne!(view(&source("return Ok(());")), view(&source("Ok(())")));
}

#[test]
fn guard_clauses_read_as_conditionals_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "fn f(p: bool) { if p { return; } a(); }",
            "fn f(p: bool) { if !p { a(); } }",
        ),
        (
            "fn f(p: bool) { for _ in [0u8] { if p { continue; } a(); } }",
            "fn f(p: bool) { for _ in [0u8] { if !p { a(); } } }",
        ),
        (
            "fn f(p: bool) { loop { if p { break; } a(); } }",
            "fn f(p: bool) { while !p { a(); } }",
        ),
    ] {
        assert_eq!(view(written), view(canonical), "{written}");
    }
}

#[test]
fn guards_before_items_or_tails_keep_their_jump_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "fn f(p: bool) { if p { return; } fn b() {} b(); }",
            "fn f(p: bool) { if !p { fn b() {} b(); } }",
        ),
        (
            "fn f(p: bool) { if p { return; } for _ in [0u8] { a(); } }",
            "fn f(p: bool) { if !p { for _ in [0u8] { a(); } } }",
        ),
    ] {
        assert_ne!(view(written), view(canonical), "{written}");
    }
}

#[test]
fn the_failure_mode_retains_guard_clauses() {
    assert_ne!(
        failing_form("fn f(p: bool) { if p { return; } a(); }"),
        failing_form("fn f(p: bool) { if !p { a(); } }")
    );
    assert_ne!(
        failing_form("fn f(p: bool) { loop { if p { break; } a(); } }"),
        failing_form("fn f(p: bool) { while !p { a(); } }")
    );
}

#[test]
fn an_observing_impl_retains_guard_clauses() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f(p: bool) {{ {body} }}\n}}")
    };
    assert_ne!(
        view(&source("if p { return; } a();")),
        view(&source("if !p { a(); }"))
    );
}

#[test]
fn match_arm_readings_hold_in_the_borrowed_view() {
    for (written, read) in [
        (
            "fn f(o: Option<u32>) -> u32 { if let Some(v) = o { v } else { 0 } }",
            "fn f(o: Option<u32>) -> u32 { match o { None => 0, Some(v) => v } }",
        ),
        (
            "fn f(x: u32) -> u32 { match x { 2 => 7, 0 => 1, 1 => 7, _ => 3 } }",
            "fn f(x: u32) -> u32 { match x { 0 => 1, 1 | 2 => 7, _ => 3 } }",
        ),
        (
            "enum E { A, B } fn f(e: E) -> u32 { match e { E::B => 1, E::A => 0 } }",
            "enum E { A, B } fn f(e: E) -> u32 { match e { E::A => 0, E::B => 1 } }",
        ),
    ] {
        assert_eq!(view(written), view(read), "{written}");
    }
}

#[test]
fn module_names_keep_arm_spellings_in_the_borrowed_view() {
    for (written, read) in [
        (
            "use crate::E::A as None; enum E { A, B } fn f(o: Option<u32>) -> u32 { match o { Some(v) => v, None => 0 } }",
            "use crate::E::A as None; enum E { A, B } fn f(o: Option<u32>) -> u32 { match o { Some(v) => v, _ => 0 } }",
        ),
        (
            "mod E { pub const A: u32 = 1; pub const B: u32 = 1; } fn f(x: u32) -> u32 { match x { E::B => 1, E::A => 0, _ => 2 } }",
            "mod E { pub const A: u32 = 1; pub const B: u32 = 1; } fn f(x: u32) -> u32 { match x { E::A => 0, E::B => 1, _ => 2 } }",
        ),
        (
            "static S: Option<u32> = None; fn f() -> u32 { if let Some(v) = S { v } else { 0 } }",
            "static S: Option<u32> = None; fn f() -> u32 { match S { Some(v) => v, _ => 0 } }",
        ),
    ] {
        assert_ne!(view(written), view(read), "{written}");
    }
}

#[test]
fn the_failure_mode_retains_match_arm_spellings() {
    assert_ne!(
        failing_form("fn f(o: Option<u32>) -> u32 { if let Some(v) = o { v } else { 0 } }"),
        failing_form("fn f(o: Option<u32>) -> u32 { match o { None => 0, Some(v) => v } }")
    );
    assert_ne!(
        failing_form("fn f(x: u32) -> u32 { match x { 1 => 7, 0 => 1, _ => 3 } }"),
        failing_form("fn f(x: u32) -> u32 { match x { 0 => 1, 1 => 7, _ => 3 } }")
    );
}

#[test]
fn an_observing_impl_retains_match_arm_spellings() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f(x: u32) -> u32 {{ {body} }}\n}}")
    };
    assert_ne!(
        view(&source("match x { 1 => 7, 0 => 1, _ => 3 }")),
        view(&source("match x { 0 => 1, 1 => 7, _ => 3 }"))
    );
}

#[test]
fn discarded_call_bindings_read_as_statements_in_the_borrowed_view() {
    assert_eq!(
        view("fn f(v: &mut Vec<u32>) { let _ = v.pop(); let _ = g(); }"),
        view("fn f(v: &mut Vec<u32>) { v.pop(); g(); }")
    );
    for (bound, dropped) in [
        (
            "fn f(v: Vec<u32>) { let _ = v; }",
            "fn f(v: Vec<u32>) { v; }",
        ),
        (
            "fn f(v: &mut Vec<u32>) { let _last = v.pop(); }",
            "fn f(v: &mut Vec<u32>) { v.pop(); }",
        ),
    ] {
        assert_ne!(view(bound), view(dropped), "{bound}");
    }
}

#[test]
fn the_failure_mode_retains_discarded_call_bindings() {
    assert_ne!(
        failing_form("fn f(v: &mut Vec<u32>) { let _ = v.pop(); }"),
        failing_form("fn f(v: &mut Vec<u32>) { v.pop(); }")
    );
}

#[test]
fn an_observing_impl_retains_discarded_call_bindings() {
    let source = |body: &str| {
        format!("struct S;\n#[observe]\nimpl S {{\n    fn f(v: &mut Vec<u32>) {{ {body} }}\n}}")
    };
    assert_ne!(view(&source("let _ = v.pop();")), view(&source("v.pop();")));
}

#[test]
fn adjacent_literal_comparisons_read_as_strict_in_the_borrowed_view() {
    assert_eq!(
        view("fn f(x: u32, y: i8) -> bool { x >= 1 && y <= 5 }"),
        view("fn f(x: u32, y: i8) -> bool { x != 0 && y < 6 }")
    );
    for (written, read) in [
        (
            "fn f(y: i8) -> bool { y > 0 }",
            "fn f(y: i8) -> bool { y != 0 }",
        ),
        (
            "fn f(z: u8) -> bool { z <= 255 }",
            "fn f(z: u8) -> bool { z < 256 }",
        ),
    ] {
        assert_ne!(view(written), view(read), "{written}");
    }
}

#[test]
fn the_failure_mode_retains_inclusive_comparisons() {
    assert_ne!(
        failing_form("fn f(x: u32) -> bool { x >= 1 }"),
        failing_form("fn f(x: u32) -> bool { x != 0 }")
    );
}

#[test]
fn integer_choices_read_as_casts_and_maxima_in_the_borrowed_view() {
    assert_eq!(
        view("fn f(x: u32, y: u32) -> u32 { if x >= y { x } else { y } }"),
        view("fn f(x: u32, y: u32) -> u32 { std::cmp::max(y, x) }")
    );
    assert_eq!(
        view("fn f(p: bool) -> u8 { if p { 1u8 } else { 0u8 } }"),
        view("fn f(p: bool) -> u8 { u8::from(p) }")
    );
    assert_ne!(
        view("fn f(x: f32, y: f32) -> f32 { if x > y { x } else { y } }"),
        view("fn f(x: f32, y: f32) -> f32 { x.max(y) }")
    );
}

#[test]
fn the_failure_mode_retains_integer_choices() {
    assert_ne!(
        failing_form("fn f(x: u32, y: u32) -> u32 { x.max(y) }"),
        failing_form("fn f(x: u32, y: u32) -> u32 { y.max(x) }")
    );
}

#[test]
fn collection_readings_hold_in_the_borrowed_view() {
    assert_eq!(
        view("fn f(v: &[u32]) -> bool { for x in v.iter() { g(x); } v.len() == 0 }"),
        view("fn f(v: &[u32]) -> bool { for x in v { g(x); } v.is_empty() }")
    );
    assert_ne!(
        view("trait Size { fn len(&self) -> usize; } fn f(v: &[u32]) -> bool { v.len() == 0 }"),
        view("trait Size { fn len(&self) -> usize; } fn f(v: &[u32]) -> bool { v.is_empty() }")
    );
}

#[test]
fn the_failure_mode_retains_collection_spellings() {
    assert_ne!(
        failing_form("fn f(v: &[u32]) -> bool { v.len() == 0 }"),
        failing_form("fn f(v: &[u32]) -> bool { v.is_empty() }")
    );
}

#[test]
fn literal_strings_and_formatting_arguments_read_alike_in_the_borrowed_view() {
    assert_eq!(
        view("fn f(n: u32) -> String { println!(\"{} {}\", \"a\", n); format!(\"x\") }"),
        view("fn f(n: u32) -> String { println!(\"a {n}\"); \"x\".to_string() }")
    );
    assert_eq!(
        view("fn f() -> String { \"\".to_owned() }"),
        view("fn f() -> String { String::new() }")
    );
    assert_ne!(
        view("fn f() { println!(\"{:?}\", \"a\"); }"),
        view("fn f() { println!(\"a\"); }")
    );
}

#[test]
fn the_failure_mode_retains_literal_formatting_arguments() {
    assert_ne!(
        failing_form("fn f() { println!(\"{}\", \"a\"); }"),
        failing_form("fn f() { println!(\"a\"); }")
    );
}

#[test]
fn bounds_outside_where_clauses_sort_in_the_borrowed_view() {
    assert_eq!(
        view("fn f(x: Box<dyn Sync + Send>, y: impl Debug + Clone) {}"),
        view("fn f(x: Box<dyn Send + Sync>, y: impl Clone + Debug) {}")
    );
    assert_eq!(
        view(
            "struct S<'a>(&'a u8);\nimpl<'a> S<'a> {\n    fn f(&self) -> Box<dyn Sync + Send> { Box::new(*self.0) }\n}"
        ),
        view(
            "struct S<'a>(&'a u8);\nimpl S<'_> {\n    fn f(&self) -> Box<dyn Send + Sync> { Box::new(*self.0) }\n}"
        )
    );
    assert_ne!(
        view("fn f(x: Box<dyn Send + Fn()>) {}"),
        view("fn f(x: Box<dyn Fn() + Send>) {}")
    );
}

#[test]
fn the_failure_mode_retains_bound_order_and_impl_lifetimes() {
    assert_ne!(
        failing_form("fn f(x: Box<dyn Sync + Send>) {}"),
        failing_form("fn f(x: Box<dyn Send + Sync>) {}")
    );
    assert_ne!(
        failing_form("struct S<'a>(&'a u8); impl<'a> S<'a> {}"),
        failing_form("struct S<'a>(&'a u8); impl S<'_> {}")
    );
}

#[test]
fn literal_casts_limits_and_power_of_two_arithmetic_read_alike_in_the_borrowed_view() {
    for (written, canonical) in [
        ("fn f() -> u64 { 5 as u64 }", "fn f() -> u64 { 5u64 }"),
        ("fn f() -> u8 { u8::MAX }", "fn f() -> u8 { 255u8 }"),
        ("fn f() -> u8 { b'a' }", "fn f() -> u8 { 97u8 }"),
        (
            "fn f(x: u32) -> u32 { x / 8 }",
            "fn f(x: u32) -> u32 { x >> 3 }",
        ),
        (
            "fn f(x: i32) -> bool { x % 2 == 0 }",
            "fn f(x: i32) -> bool { x & 1 == 0 }",
        ),
    ] {
        assert_eq!(view(written), view(canonical), "{written}");
    }
    assert_ne!(
        view("fn f(x: i32) -> i32 { x / 2 }"),
        view("fn f(x: i32) -> i32 { x >> 1 }")
    );
}

#[test]
fn the_failure_mode_retains_literal_casts_and_power_of_two_arithmetic() {
    assert_ne!(
        failing_form("fn f() -> u64 { 5 as u64 }"),
        failing_form("fn f() -> u64 { 5u64 }")
    );
    assert_ne!(
        failing_form("fn f(x: u32) -> u32 { x / 8 }"),
        failing_form("fn f(x: u32) -> u32 { x >> 3 }")
    );
}

#[test]
fn literal_disjunctions_read_as_one_chain_in_the_borrowed_view() {
    for (written, canonical) in [
        (
            "fn f(x: u32) -> bool { matches!(x, 2 | 1) }",
            "fn f(x: u32) -> bool { x == 1 || x == 2 }",
        ),
        (
            "fn f(x: u8) -> bool { (1..=5).contains(&x) }",
            "fn f(x: u8) -> bool { x >= 1 && x <= 5 }",
        ),
    ] {
        assert_eq!(view(written), view(canonical), "{written}");
    }
    assert_ne!(
        view("fn f(x: u32) -> bool { matches!(x, 1 | 2 if x > 1) }"),
        view("fn f(x: u32) -> bool { x == 1 || x == 2 }")
    );
}

#[test]
fn the_failure_mode_retains_literal_disjunction_spellings() {
    assert_ne!(
        failing_form("fn f(x: u32) -> bool { matches!(x, 1 | 2) }"),
        failing_form("fn f(x: u32) -> bool { x == 1 || x == 2 }")
    );
}

#[test]
fn return_types_type_literal_tails_in_the_borrowed_view() {
    for (written, canonical) in [
        ("fn f() -> u32 { 2 + 3 }", "fn f() -> u32 { 5u32 }"),
        (
            "fn f(p: bool) -> u8 { if p { return 7; } 9 }",
            "fn f(p: bool) -> u8 { if p { return 7u8; } 9u8 }",
        ),
    ] {
        assert_eq!(view(written), view(canonical), "{written}");
    }
    assert_ne!(
        view("fn f() -> impl Copy { 5 }"),
        view("fn f() -> impl Copy { 5u32 }")
    );
}

#[test]
fn the_failure_mode_retains_untyped_literal_tails() {
    assert_ne!(
        failing_form("fn f() -> u32 { 5 }"),
        failing_form("fn f() -> u32 { 5u32 }")
    );
}

#[test]
fn a_returned_value_built_from_closed_literal_arithmetic_reads_as_its_tail() {
    let original = "fn f(red: u32, green: u32, blue: u32, alpha: u32, opaque: bool) -> u32 {
        let rgb = ((red & 0xffu32) << 16u32) | ((green & 0xffu32) << 8u32) | (blue & 0xffu32);
        let a = if opaque { 0xffu32 } else { alpha & 0xffu32 };
        let visible = if a == 0u32 { 0u32 } else { rgb };
        (a << 24u32) | visible
    }";
    let rewritten = "fn f(x: u32, y: u32, z: u32, w: u32, solid: bool) -> u32 {
        let unused = 17u32;
        let lo = 255u32 & z;
        let mid = (y & 255u32) << (2u32 * 4u32);
        let packed = lo | mid | (x & 0xffu32) << 16u32;
        let top = if !solid { w & 255u32 } else { 255u32 };
        let shown = if top == 0u32 { 0u32 } else { packed };
        return top << 24u32 | shown;
    }";
    assert_eq!(view(original), view(rewritten));
}
