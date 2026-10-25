//! Temporary-scope boundaries across return normalization.

#[path = "support/view.rs"]
mod view;

use syn_canon::ContextKind;
use view::view_of;

const DROP_GUARD: &str = r#"
use std::io::Write as _;
struct D(&'static str);
impl D { fn value(&self) -> u32 { 0 } }
impl Drop for D {
    fn drop(&mut self) { let _ = writeln!(std::io::stdout(), "drop {}", self.0); }
}
"#;

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn compare(left: &str, right: &str, equal: bool) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        let a = canonicalize(parse(left));
        let b = canonicalize(parse(right));
        assert_eq!(a == b, equal, "{left}\n{right}\n{a:?}\n{b:?}");
    }
}

#[test]
fn destructor_temporaries_keep_returns_at_every_function_boundary() {
    let written = "let local = D(\"local\"); return D(\"temp\").value();";
    let tail = "let local = D(\"local\"); D(\"temp\").value()";
    for template in [
        "fn f() -> u32 { BODY }",
        "struct S; impl S { fn f(&self) -> u32 { BODY } }",
        "trait T { fn f(&self) -> u32 { BODY } }",
        "fn f() { let _ = || -> u32 { BODY }; }",
        "fn f() { let _ = async { BODY }; }",
    ] {
        compare(
            &format!("{DROP_GUARD} {}", template.replace("BODY", written)),
            &format!("{DROP_GUARD} {}", template.replace("BODY", tail)),
            false,
        );
    }
}

#[test]
fn borrowed_temporaries_keep_compilation_distinctions() {
    compare(
        "use std::cell::RefCell; fn f() -> u32 { let value = RefCell::new(7u32); return *value.borrow(); }",
        "use std::cell::RefCell; fn f() -> u32 { let value = RefCell::new(7u32); *value.borrow() }",
        false,
    );
}

#[test]
fn unknown_calls_keep_their_return_boundary() {
    for expression in [
        "make()",
        "receiver.read()",
        "*borrowed",
        "left + right",
        "inferred",
    ] {
        compare(
            &format!("fn f() -> u32 {{ return {expression}; }}"),
            &format!("fn f() -> u32 {{ {expression} }}"),
            false,
        );
    }
}

#[test]
fn nested_forwarded_calls_keep_their_return_boundary() {
    for expression in [
        "{ return D(\"temp\").value(); }",
        "// SAFETY: The block contains only safe operations.\nunsafe { return D(\"temp\").value(); }",
        "if condition { return D(\"temp\").value(); } else { 1u32 }",
        "match condition { true => return D(\"temp\").value(), false => 1u32 }",
    ] {
        let tail = expression
            .replace("return D(\"temp\").value();", "D(\"temp\").value()")
            .replace("return D(\"temp\").value(),", "D(\"temp\").value(),");
        compare(
            &format!(
                "{DROP_GUARD} fn f(condition: bool) -> u32 {{ let local = D(\"local\"); {expression} }}"
            ),
            &format!(
                "{DROP_GUARD} fn f(condition: bool) -> u32 {{ let local = D(\"local\"); {tail} }}"
            ),
            false,
        );
    }
}

#[test]
fn match_scrutinee_temporaries_protect_primitive_arm_returns() {
    compare(
        &format!(
            "{DROP_GUARD} fn f() -> u32 {{ let local = D(\"local\"); match D(\"temp\").value() {{ 0 => return 1u32, _ => return 2u32 }} }}"
        ),
        &format!(
            "{DROP_GUARD} fn f() -> u32 {{ let local = D(\"local\"); match D(\"temp\").value() {{ 0 => 1u32, _ => 2u32 }} }}"
        ),
        false,
    );
}

#[test]
fn unproven_conditions_protect_forwarded_primitive_returns() {
    compare(
        "fn f() -> u32 { if test() { return 1; } else { return 2; } }",
        "fn f() -> u32 { if test() { 1 } else { 2 } }",
        false,
    );
    compare(
        "fn f() -> u32 { return match make() { _ => 1 }; }",
        "fn f() -> u32 { match make() { _ => 1 } }",
        false,
    );
}

#[test]
fn proven_primitive_returns_keep_supported_equivalences() {
    for (written, tail) in [
        ("fn f() -> u32 { return 1; }", "fn f() -> u32 { 1 }"),
        (
            "fn f(x: u32) -> u32 { return x; }",
            "fn f(x: u32) -> u32 { x }",
        ),
        (
            "fn f(x: u32) -> u32 { return x & 15u32; }",
            "fn f(x: u32) -> u32 { x & 15u32 }",
        ),
        (
            "fn f() -> (u32, bool) { return (1u32, true); }",
            "fn f() -> (u32, bool) { (1u32, true) }",
        ),
        (
            "fn f(x: bool) -> u32 { if x { return 1; } else { return 2; } }",
            "fn f(x: bool) -> u32 { if x { 1 } else { 2 } }",
        ),
        (
            "fn f(x: u32) -> u32 { match x { 0 => return 1, _ => return 2 } }",
            "fn f(x: u32) -> u32 { match x { 0 => 1, _ => 2 } }",
        ),
        (
            "fn f() -> u32 { return { unknown(); return 2; }; }",
            "fn f() -> u32 { { unknown(); 2 } }",
        ),
    ] {
        compare(written, tail, true);
    }
    compare(
        &format!("{DROP_GUARD} fn f() -> u32 {{ let local = D(\"local\"); return 1; }}"),
        &format!("{DROP_GUARD} fn f() -> u32 {{ let local = D(\"local\"); 1 }}"),
        true,
    );
}

#[test]
fn observed_return_spelling_stays_distinct() {
    for template in [
        "#[observe] fn f() -> u32 { BODY }",
        "fn f() { observe!({ BODY }); }",
    ] {
        compare(
            &template.replace("BODY", "return 1u32;"),
            &template.replace("BODY", "1u32"),
            false,
        );
    }
}

#[test]
fn views_and_lexical_bodies_keep_temporary_return_scopes() {
    let source = format!(
        "{DROP_GUARD} fn first() -> u32 {{ let local = D(\"local\"); return D(\"temp\").value(); }} fn second() -> u32 {{ let local = D(\"local\"); D(\"temp\").value() }}"
    );
    let file = parse(&source);
    assert_ne!(view_of(&file, "first"), view_of(&file, "second"));
    let mut bodies = Vec::new();
    syn_canon::contexts(&file, &mut |context, form| {
        if context.kind == ContextKind::FunctionBody && matches!(context.item, "first" | "second") {
            bodies.push(form);
        }
    });
    assert_eq!(bodies.len(), 2);
    assert_ne!(bodies[0], bodies[1]);
}

#[test]
fn shadowing_discards_outer_primitive_proofs() {
    for body in [
        "let x = make(); return x;",
        "let x: Unknown = make(); return x;",
        "type u32 = Unknown; let x: u32 = make(); return x;",
        "{ const x: Unknown = make(); return x; }",
        "{ use unknown::x; return x; }",
        "{ use unknown::*; return x; }",
    ] {
        compare(
            &format!("fn f(x: u32) -> Unknown {{ {body} }}"),
            &format!(
                "fn f(x: u32) -> Unknown {{ {} }}",
                body.replace("return x;", "x")
            ),
            false,
        );
    }
}

#[test]
fn opaque_statement_macros_do_not_supply_primitive_proofs() {
    compare(
        "macro_rules! shadow { () => { let x = make(); }; } fn f(x: u32) -> u32 { shadow!(); return x + x; }",
        "macro_rules! shadow { () => { let x = make(); }; } fn f(x: u32) -> u32 { shadow!(); x + x }",
        false,
    );
}

#[test]
fn guarded_matches_require_temporary_free_guards() {
    compare(
        "fn f(x: u32, condition: bool) -> u32 { match x { _ if condition => return 1, _ => return 2 } }",
        "fn f(x: u32, condition: bool) -> u32 { match x { _ if condition => 1, _ => 2 } }",
        true,
    );
    compare(
        "fn f(x: u32) -> u32 { match x { _ if unknown() => return 1, _ => return 2 } }",
        "fn f(x: u32) -> u32 { match x { _ if unknown() => 1, _ => 2 } }",
        false,
    );
}

#[test]
fn unit_returns_keep_preceding_temporary_boundaries() {
    compare(
        "fn f() { unknown(); return; }",
        "fn f() { unknown(); }",
        true,
    );
    compare(
        "fn f() { unknown(); return; }",
        "fn f() { unknown() }",
        false,
    );
    compare("fn f() { 1; }", "fn f() { 1 }", false);
    for expression in ["x = 1", "x <<= 1"] {
        compare(
            &format!("fn f(mut x: u32) {{ {expression}; }}"),
            &format!("fn f(mut x: u32) {{ {expression} }}"),
            true,
        );
    }
    compare(
        &format!(
            "{DROP_GUARD} impl D {{ fn unit(&self) {{}} }} fn f() {{ let local = D(\"local\"); {{ D(\"temp\").unit() }} return; }}"
        ),
        &format!(
            "{DROP_GUARD} impl D {{ fn unit(&self) {{}} }} fn f() {{ let local = D(\"local\"); {{ D(\"temp\").unit() }} }}"
        ),
        false,
    );
}

#[test]
fn observed_arm_blocks_keep_their_spelling() {
    compare(
        "fn f(x: u32) -> u32 { match x { #[observe] _ => { 1 } } }",
        "fn f(x: u32) -> u32 { match x { #[observe] _ => 1 } }",
        false,
    );
}

#[test]
fn local_primitive_values_keep_return_equivalence() {
    for local in ["let value = 1u32;", "let value: u32 = unknown();"] {
        compare(
            &format!("fn f() -> u32 {{ {local} return value; }}"),
            &format!("fn f() -> u32 {{ {local} value }}"),
            true,
        );
    }
}

#[test]
fn oversized_return_boundaries_keep_their_spelling() {
    let prefix = "unknown();".repeat(16_385);
    for template in [
        "fn f() -> u32 { BODY }",
        "fn f() { let _ = || -> u32 { BODY }; }",
    ] {
        compare(
            &template.replace("BODY", &format!("{prefix} return 1;")),
            &template.replace("BODY", &format!("{prefix} 1")),
            false,
        );
    }
}

#[test]
fn observed_arms_protect_nested_return_boundaries() {
    compare(
        "fn f(x: u32) -> u32 { match x { #[observe] _ => { let _ = || -> u32 { return 1; }; 2 } } }",
        "fn f(x: u32) -> u32 { match x { #[observe] _ => { let _ = || -> u32 { 1 }; 2 } } }",
        false,
    );
}

#[test]
fn locally_rebound_standard_macros_discard_primitive_proofs() {
    for body in [
        "macro_rules! println { () => { let x = make(); }; } println!();",
        "use custom::println; println!();",
    ] {
        let prefix = "macro_rules! replacement { () => { let x = make(); }; } pub(crate) use replacement; mod custom { pub(crate) use crate::replacement as println; }";
        compare(
            &format!("{prefix} fn f(x: u32) -> u32 {{ {body} return x + x; }}"),
            &format!("{prefix} fn f(x: u32) -> u32 {{ {body} x + x }}"),
            false,
        );
    }
}

#[test]
fn rebound_derive_macros_protect_nested_return_boundaries() {
    compare(
        "use custom::Debug; #[derive(Debug)] struct S { data: [u8; { let _ = || -> u32 { return 1; }; 0usize }] }",
        "use custom::Debug; #[derive(Debug)] struct S { data: [u8; { let _ = || -> u32 { 1 }; 0usize }] }",
        false,
    );
}

#[test]
fn bare_unit_closure_returns_match_unit_values() {
    compare(
        "fn f() { let _ = || return; }",
        "fn f() { let _ = || (); }",
        true,
    );
}

#[test]
fn unit_value_semicolons_do_not_change_return_scopes() {
    compare("fn f() { (); }", "fn f() { () }", true);
    compare(
        "fn f(mut x: i32) { x = -1; }",
        "fn f(mut x: i32) { x = -1 }",
        true,
    );
}

#[test]
fn attributed_assignment_operands_keep_temporary_boundaries() {
    for statement in ["#[observe] x = 1", "x = #[observe] 1"] {
        compare(
            &format!("fn f(mut x: u32) {{ {statement}; }}"),
            &format!("fn f(mut x: u32) {{ {statement} }}"),
            false,
        );
    }
}

#[test]
fn guard_bindings_do_not_inherit_outer_primitive_proofs() {
    compare(
        "fn f(gate: bool) -> u32 { match true { gate if gate => return 1, _ => return 2 } }",
        "fn f(gate: bool) -> u32 { match true { gate if gate => 1, _ => 2 } }",
        false,
    );
}

#[test]
fn attributed_guard_patterns_keep_forwarded_returns() {
    struct Observe;
    impl syn::visit_mut::VisitMut for Observe {
        fn visit_pat_ident_mut(&mut self, pattern: &mut syn::PatIdent) {
            pattern.attrs.push(syn::parse_quote!(#[observe]));
        }
    }
    let observed = |source: &str| {
        let mut file = parse(source);
        syn::visit_mut::VisitMut::visit_file_mut(&mut Observe, &mut file);
        file
    };
    let written = "fn f() -> u32 { match (1u32,) { (value,) if true => return 1, _ => return 2 } }";
    let tail = "fn f() -> u32 { match (1u32,) { (value,) if true => 1, _ => 2 } }";
    compare(written, tail, true);
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(observed(written)),
            canonicalize(observed(tail))
        );
    }
}

#[test]
fn oversized_attribute_values_cannot_supply_a_scalar_proof() {
    let values = "1u32,".repeat(16_385);
    compare(
        &format!("fn f(x: u32) -> u32 {{ return x + #[observe = [{values}]] x; }}"),
        &format!("fn f(x: u32) -> u32 {{ x + #[observe = [{values}]] x }}"),
        false,
    );
}

#[test]
fn invisible_expression_groups_preserve_tail_scope_proofs() {
    struct Groups;
    impl syn::visit_mut::VisitMut for Groups {
        fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
            syn::visit_mut::visit_expr_mut(self, expr);
            if matches!(expr, syn::Expr::Return(_)) {
                return;
            }
            let inner =
                core::mem::replace(expr, syn::Expr::Verbatim(proc_macro2::TokenStream::new()));
            *expr = syn::Expr::Group(syn::ExprGroup {
                attrs: Vec::new(),
                group_token: syn::token::Group::default(),
                expr: Box::new(inner),
            });
        }
    }
    let grouped = |source: &str| {
        let mut file = parse(source);
        syn::visit_mut::VisitMut::visit_file_mut(&mut Groups, &mut file);
        file
    };
    for (written, tail, equal) in [
        (
            "fn f(x: u32) -> u32 { return x }",
            "fn f(x: u32) -> u32 { x }",
            true,
        ),
        (
            "fn f(mut x: i32) { x = -1; }",
            "fn f(mut x: i32) { x = -1 }",
            true,
        ),
        ("fn f() { (); }", "fn f() { () }", true),
        (
            "fn f(gate: bool) -> u32 { if gate { return 1 } else { return 2 } }",
            "fn f(gate: bool) -> u32 { if gate { 1 } else { 2 } }",
            true,
        ),
        (
            "fn f() -> u32 { return make() }",
            "fn f() -> u32 { make() }",
            false,
        ),
    ] {
        for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
            let left = canonicalize(grouped(written));
            let right = canonicalize(grouped(tail));
            assert_eq!(
                left == right,
                equal,
                "{written}\n{tail}\n{left:?}\n{right:?}"
            );
        }
    }
}
