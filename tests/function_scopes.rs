//! A function view keeps the syntax an enclosing attribute macro reads, the derive input of a
//! std derive name its module may bind to another macro, and the format string of a call its
//! scopes do not prove to be the std formatting macro.

use syn_canon::{CanonicalForm, SourceContext};

/// The view of the function `f`, free or in an impl, canonicalized in its source context.
fn view(source: &str) -> CanonicalForm {
    let file: syn::File = syn::parse_str(source).unwrap_or_else(|e| panic!("{source}: {e}"));
    let context = SourceContext::new(core::iter::once((&[][..], &file)));
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
    let context = SourceContext::new([(&[][..], &root), (&path[..], &module)]);
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
