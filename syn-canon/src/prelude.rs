//! Owned string literals and empty vectors under the standard prelude.
//!
//! With the prelude active, `"…".to_owned()` and `String::from("…")` build the same `String` as
//! `"…".to_string()`, and `vec![]` expands to `Vec::new()`. A string literal receiver resolves
//! `to_string` and `to_owned` at its first candidate step, where a user trait method would make
//! the call ambiguous, so compiling source reaches the standard methods.

use alloc::boxed::Box;
use alloc::vec::Vec;
use syn::visit_mut::VisitMut;

use crate::alpha::Renamer;

/// The names the spellings resolve through the prelude.
const STRING_NAMES: [&str; 3] = ["String", "ToString", "ToOwned"];

/// Read the owned string literals and empty vectors of `block`, nested items aside, when the
/// scopes in view leave the prelude names unclaimed and the block claims none of them.
pub(crate) fn normalize(renamer: &Renamer<'_>, block: &mut syn::Block) {
    let mut claims = Claims::default();
    syn::visit::Visit::visit_block(&mut claims, block);
    let mut fold = Fold {
        renamer,
        strings: !claims.strings
            && renamer.owned_methods()
            && STRING_NAMES.iter().all(|name| renamer.type_unclaimed(name)),
        vectors: !claims.vectors && renamer.type_unclaimed("Vec"),
    };
    if fold.strings || fold.vectors {
        fold.visit_block_mut(block);
    }
}

/// The prelude names an item, import, generic parameter or glob inside a block may claim.
#[derive(Default)]
struct Claims {
    strings: bool,
    vectors: bool,
}

impl Claims {
    fn claim(&mut self, ident: &syn::Ident) {
        let name = crate::scope::ident_name(ident);
        self.strings |= STRING_NAMES.contains(&name.as_str());
        self.vectors |= name == "Vec" || name == "vec";
    }
}

impl syn::visit::Visit<'_> for Claims {
    fn visit_item(&mut self, item: &syn::Item) {
        if let Some(ident) = item_ident(item) {
            self.claim(ident);
        }
        syn::visit::visit_item(self, item);
    }

    fn visit_use_name(&mut self, name: &syn::UseName) {
        self.claim(&name.ident);
    }

    fn visit_use_rename(&mut self, rename: &syn::UseRename) {
        self.claim(&rename.rename);
    }

    fn visit_use_glob(&mut self, _: &syn::UseGlob) {
        self.strings = true;
        self.vectors = true;
    }

    fn visit_type_param(&mut self, param: &syn::TypeParam) {
        self.claim(&param.ident);
        syn::visit::visit_type_param(self, param);
    }
}

fn item_ident(item: &syn::Item) -> Option<&syn::Ident> {
    match item {
        syn::Item::Struct(item) => Some(&item.ident),
        syn::Item::Enum(item) => Some(&item.ident),
        syn::Item::Union(item) => Some(&item.ident),
        syn::Item::Trait(item) => Some(&item.ident),
        syn::Item::TraitAlias(item) => Some(&item.ident),
        syn::Item::Type(item) => Some(&item.ident),
        syn::Item::Mod(item) => Some(&item.ident),
        syn::Item::Macro(item) => item.ident.as_ref(),
        syn::Item::ExternCrate(item) => Some(
            item.rename
                .as_ref()
                .map_or(&item.ident, |(_, rename)| rename),
        ),
        _ => None,
    }
}

struct Fold<'a, 'env> {
    renamer: &'a Renamer<'env>,
    strings: bool,
    vectors: bool,
}

impl Fold<'_, '_> {
    fn fold(&self, expr: &mut syn::Expr) {
        match expr {
            syn::Expr::MethodCall(call)
                if self.strings
                    && call.attrs.is_empty()
                    && call.turbofish.is_none()
                    && call.args.is_empty()
                    && crate::scope::ident_name(&call.method) == "to_owned"
                    && string_literal(&call.receiver) =>
            {
                call.method = syn::Ident::new("to_string", call.method.span());
            }
            syn::Expr::Call(call)
                if self.strings
                    && call.attrs.is_empty()
                    && call.args.len() == 1
                    && string_literal(&call.args[0])
                    && path_is(&call.func, "String", "from") =>
            {
                let receiver = core::mem::replace(
                    &mut call.args[0],
                    syn::Expr::Verbatim(proc_macro2::TokenStream::new()),
                );
                *expr = syn::Expr::MethodCall(syn::ExprMethodCall {
                    attrs: Vec::new(),
                    receiver: Box::new(receiver),
                    dot_token: syn::token::Dot::default(),
                    method: syn::Ident::new("to_string", proc_macro2::Span::call_site()),
                    turbofish: None,
                    paren_token: syn::token::Paren::default(),
                    args: syn::punctuated::Punctuated::new(),
                });
            }
            syn::Expr::Macro(call)
                if self.vectors
                    && call.attrs.is_empty()
                    && call.mac.tokens.is_empty()
                    && self.renamer.standard_vec(&call.mac.path) =>
            {
                *expr = syn::Expr::Call(syn::ExprCall {
                    attrs: Vec::new(),
                    func: Box::new(syn::Expr::Path(syn::ExprPath {
                        attrs: Vec::new(),
                        qself: None,
                        path: syn::Path {
                            leading_colon: None,
                            segments: ["Vec", "new"]
                                .into_iter()
                                .map(|name| {
                                    syn::PathSegment::from(syn::Ident::new(
                                        name,
                                        proc_macro2::Span::call_site(),
                                    ))
                                })
                                .collect(),
                        },
                    })),
                    paren_token: syn::token::Paren::default(),
                    args: syn::punctuated::Punctuated::new(),
                });
            }
            _ => {}
        }
    }
}

impl VisitMut for Fold<'_, '_> {
    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        if !crate::drift::expr_attrs(expr).is_empty() {
            return;
        }
        syn::visit_mut::visit_expr_mut(self, expr);
        self.fold(expr);
    }

    fn visit_item_mut(&mut self, _: &mut syn::Item) {}

    fn visit_local_mut(&mut self, local: &mut syn::Local) {
        if local.attrs.is_empty() {
            syn::visit_mut::visit_local_mut(self, local);
        }
    }

    fn visit_arm_mut(&mut self, arm: &mut syn::Arm) {
        if arm.attrs.is_empty() {
            syn::visit_mut::visit_arm_mut(self, arm);
        }
    }
}

/// Whether `expr` is an unattributed, unsuffixed string literal.
fn string_literal(expr: &syn::Expr) -> bool {
    matches!(
        expr,
        syn::Expr::Lit(syn::ExprLit { attrs, lit: syn::Lit::Str(literal) })
            if attrs.is_empty() && literal.suffix().is_empty()
    )
}

/// Whether `expr` is the unattributed relative path `head::tail`.
fn path_is(expr: &syn::Expr, head: &str, tail: &str) -> bool {
    let syn::Expr::Path(path) = expr else {
        return false;
    };
    let mut segments = path.path.segments.iter();
    path.attrs.is_empty()
        && path.qself.is_none()
        && path.path.leading_colon.is_none()
        && matches!(
            (segments.next(), segments.next(), segments.next()),
            (Some(first), Some(second), None)
                if first.arguments.is_none()
                    && second.arguments.is_none()
                    && crate::scope::ident_name(&first.ident) == head
                    && crate::scope::ident_name(&second.ident) == tail
        )
}
