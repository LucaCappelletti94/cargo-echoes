//! Iteration and emptiness tests over bindings of proven standard collection types.
//!
//! A binding declared as `Vec<T>`, `[T; N]`, `[T]`, `String` or `str`, behind at most one
//! reference, reaches the standard `iter`, `iter_mut`, `into_iter`, `len` and `is_empty` when no
//! trait in view declares those names. `for x in v.iter()` then walks what `for x in &v` walks,
//! and `v.len() == 0` holds exactly when `v.is_empty()` does.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use syn::visit_mut::VisitMut;

use crate::alpha::Renamer;
use crate::scope::ident_name;

/// Read the collection loops and emptiness tests of the body `block` of a function with the
/// signature `sig`, nested items aside.
pub(crate) fn normalize(renamer: &Renamer<'_>, sig: &syn::Signature, block: &mut syn::Block) {
    let mut claims = Claims::default();
    syn::visit::Visit::visit_block(&mut claims, block);
    let unclaimed = |name: &str| !claims.names.contains(name) && renamer.type_unclaimed(name);
    let mut fold = Fold {
        vectors: !claims.glob && unclaimed("Vec"),
        strings: !claims.glob && unclaimed("String"),
        slices: !claims.glob && unclaimed("str"),
        scopes: alloc::vec![BTreeMap::new()],
    };
    for input in &sig.inputs {
        if let syn::FnArg::Typed(typed) = input {
            let kind = fold.collection(&typed.ty);
            fold.bind(&typed.pat, kind);
        }
    }
    fold.visit_block_mut(block);
}

/// The type names an item or import inside the body declares.
#[derive(Default)]
struct Claims {
    names: BTreeSet<String>,
    glob: bool,
}

impl syn::visit::Visit<'_> for Claims {
    fn visit_item(&mut self, item: &syn::Item) {
        if let Some(ident) = crate::alpha::type_item_ident(item) {
            self.names.insert(ident_name(ident));
        }
        syn::visit::visit_item(self, item);
    }

    fn visit_use_name(&mut self, name: &syn::UseName) {
        self.names.insert(ident_name(&name.ident));
    }

    fn visit_use_rename(&mut self, rename: &syn::UseRename) {
        self.names.insert(ident_name(&rename.rename));
    }

    fn visit_use_glob(&mut self, _: &syn::UseGlob) {
        self.glob = true;
    }

    fn visit_type_param(&mut self, param: &syn::TypeParam) {
        self.names.insert(ident_name(&param.ident));
        syn::visit::visit_type_param(self, param);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Vec,
    Array,
    Slice,
    String,
    Str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Owned,
    Shared,
    Mut,
}

#[derive(Clone, Copy)]
struct Collection {
    kind: Kind,
    access: Access,
}

struct Fold {
    vectors: bool,
    strings: bool,
    slices: bool,
    /// The bindings in view, a proven collection or `None` for any other binding.
    scopes: Vec<BTreeMap<String, Option<Collection>>>,
}

impl Fold {
    /// The collection a declared type proves, behind at most one reference.
    fn collection(&self, ty: &syn::Type) -> Option<Collection> {
        let (access, ty) = match ty {
            syn::Type::Reference(reference) if reference.mutability.is_some() => {
                (Access::Mut, reference.elem.as_ref())
            }
            syn::Type::Reference(reference) => (Access::Shared, reference.elem.as_ref()),
            ty => (Access::Owned, ty),
        };
        let kind = match ty {
            syn::Type::Array(_) => Kind::Array,
            syn::Type::Slice(_) => Kind::Slice,
            syn::Type::Path(path) if path.qself.is_none() && path.path.leading_colon.is_none() => {
                let [segment] = path.path.segments.iter().collect::<Vec<_>>()[..] else {
                    return None;
                };
                match (ident_name(&segment.ident).as_str(), &segment.arguments) {
                    ("Vec", syn::PathArguments::AngleBracketed(args))
                        if self.vectors && args.args.len() == 1 =>
                    {
                        Kind::Vec
                    }
                    ("String", syn::PathArguments::None) if self.strings => Kind::String,
                    ("str", syn::PathArguments::None) if self.slices => Kind::Str,
                    _ => return None,
                }
            }
            _ => return None,
        };
        Some(Collection { kind, access })
    }

    /// Bind the names of `pat`, a lone identifier to `kind` and every other name to `None`.
    fn bind(&mut self, pat: &syn::Pat, kind: Option<Collection>) {
        struct Names<'s>(
            &'s mut BTreeMap<String, Option<Collection>>,
            Option<Collection>,
        );
        impl syn::visit::Visit<'_> for Names<'_> {
            fn visit_pat_ident(&mut self, ident: &syn::PatIdent) {
                let kind = self
                    .1
                    .filter(|_| ident.by_ref.is_none() && ident.subpat.is_none());
                self.0.insert(ident_name(&ident.ident), kind);
                syn::visit::visit_pat_ident(self, ident);
            }

            fn visit_expr(&mut self, _: &syn::Expr) {}
        }
        let kind = kind.filter(|_| matches!(pat, syn::Pat::Ident(_)));
        let scope = self.scopes.last_mut().expect("a scope");
        syn::visit::Visit::visit_pat(&mut Names(scope, kind), pat);
    }

    /// The collection the single-identifier path `expr` names.
    fn binding(&self, expr: &syn::Expr) -> Option<Collection> {
        let syn::Expr::Path(path) = expr else {
            return None;
        };
        let name = ident_name(path.path.get_ident().filter(|_| path.qself.is_none())?);
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&name))
            .copied()
            .flatten()
    }

    /// Read `v.iter()`, `v.iter_mut()` and `v.into_iter()` as the loop's iterable.
    fn iterable(&self, expr: &mut syn::Expr) {
        let syn::Expr::MethodCall(call) = expr else {
            return;
        };
        if !call.attrs.is_empty() || call.turbofish.is_some() || !call.args.is_empty() {
            return;
        }
        let Some(collection) = self.binding(&call.receiver) else {
            return;
        };
        if !matches!(collection.kind, Kind::Vec | Kind::Array | Kind::Slice) {
            return;
        }
        let borrow = match (ident_name(&call.method).as_str(), collection.access) {
            ("iter", Access::Owned) => Some(false),
            ("iter_mut", Access::Owned) => Some(true),
            ("iter", Access::Shared) | ("iter_mut", Access::Mut) => None,
            ("into_iter", Access::Owned) if collection.kind == Kind::Vec => None,
            _ => return,
        };
        let receiver = core::mem::replace(
            call.receiver.as_mut(),
            syn::Expr::Verbatim(proc_macro2::TokenStream::new()),
        );
        *expr = match borrow {
            Some(mutable) => syn::Expr::Reference(syn::ExprReference {
                attrs: Vec::new(),
                and_token: syn::token::And::default(),
                mutability: mutable.then(syn::token::Mut::default),
                expr: Box::new(receiver),
            }),
            None => receiver,
        };
    }

    /// Read `v.len() == 0`, `v.len() < 1` and their mirrors as `v.is_empty()`, and
    /// `v.len() != 0`, `v.len() > 0`, `v.len() >= 1` and their mirrors as `!v.is_empty()`.
    fn emptiness(&self, expr: &mut syn::Expr) {
        let syn::Expr::Binary(binary) = expr else {
            return;
        };
        let (empty, receiver) = if let Some(receiver) = self.length(&binary.left)
            && let Some(literal) = small_literal(&binary.right)
        {
            match (binary.op, literal) {
                (syn::BinOp::Eq(_), 0) | (syn::BinOp::Lt(_), 1) => (true, receiver),
                (syn::BinOp::Ne(_) | syn::BinOp::Gt(_), 0) | (syn::BinOp::Ge(_), 1) => {
                    (false, receiver)
                }
                _ => return,
            }
        } else if let Some(receiver) = self.length(&binary.right)
            && let Some(literal) = small_literal(&binary.left)
        {
            match (binary.op, literal) {
                (syn::BinOp::Eq(_), 0) | (syn::BinOp::Gt(_), 1) => (true, receiver),
                (syn::BinOp::Ne(_) | syn::BinOp::Lt(_), 0) | (syn::BinOp::Le(_), 1) => {
                    (false, receiver)
                }
                _ => return,
            }
        } else {
            return;
        };
        let test = syn::Expr::MethodCall(syn::ExprMethodCall {
            attrs: Vec::new(),
            receiver: Box::new(receiver),
            dot_token: syn::token::Dot::default(),
            method: syn::Ident::new("is_empty", proc_macro2::Span::call_site()),
            turbofish: None,
            paren_token: syn::token::Paren::default(),
            args: syn::punctuated::Punctuated::new(),
        });
        *expr = if empty {
            test
        } else {
            syn::Expr::Unary(syn::ExprUnary {
                attrs: Vec::new(),
                op: syn::UnOp::Not(syn::token::Not::default()),
                expr: Box::new(test),
            })
        };
    }

    /// The receiver of `v.len()` over a proven collection binding.
    fn length(&self, expr: &syn::Expr) -> Option<syn::Expr> {
        let syn::Expr::MethodCall(call) = expr else {
            return None;
        };
        (call.attrs.is_empty()
            && call.turbofish.is_none()
            && call.args.is_empty()
            && call.method == "len"
            && self.binding(&call.receiver).is_some())
        .then(|| call.receiver.as_ref().clone())
    }

    fn scoped(&mut self, visit: impl FnOnce(&mut Self)) {
        self.scopes.push(BTreeMap::new());
        visit(self);
        self.scopes.pop();
    }
}

/// The value of an unattributed `0` or `1` literal, unsuffixed or `usize`.
fn small_literal(expr: &syn::Expr) -> Option<u8> {
    let syn::Expr::Lit(syn::ExprLit {
        attrs,
        lit: syn::Lit::Int(int),
    }) = expr
    else {
        return None;
    };
    match (
        attrs.is_empty() && matches!(int.suffix(), "" | "usize"),
        int.base10_digits(),
    ) {
        (true, "0") => Some(0),
        (true, "1") => Some(1),
        _ => None,
    }
}

impl VisitMut for Fold {
    fn visit_item_mut(&mut self, _: &mut syn::Item) {}

    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        self.scoped(|fold| {
            for stmt in &mut block.stmts {
                fold.visit_stmt_mut(stmt);
            }
        });
    }

    fn visit_local_mut(&mut self, local: &mut syn::Local) {
        if let Some(init) = &mut local.init {
            self.visit_expr_mut(&mut init.expr);
            if let Some((_, diverge)) = &mut init.diverge {
                self.visit_expr_mut(diverge);
            }
        }
        let (pat, kind) = match &local.pat {
            syn::Pat::Type(typed) if local.attrs.is_empty() => {
                (typed.pat.as_ref(), self.collection(&typed.ty))
            }
            pat => (pat, None),
        };
        self.bind(pat, kind);
    }

    fn visit_expr_closure_mut(&mut self, closure: &mut syn::ExprClosure) {
        self.scoped(|fold| {
            for input in &closure.inputs {
                let (pat, kind) = match input {
                    syn::Pat::Type(typed) => (typed.pat.as_ref(), fold.collection(&typed.ty)),
                    pat => (pat, None),
                };
                fold.bind(pat, kind);
            }
            fold.visit_expr_mut(&mut closure.body);
        });
    }

    fn visit_arm_mut(&mut self, arm: &mut syn::Arm) {
        self.scoped(|fold| {
            fold.bind(&arm.pat, None);
            if let syn::Pat::Guard(guard) = &mut arm.pat {
                fold.visit_expr_mut(&mut guard.guard);
            }
            fold.visit_expr_mut(&mut arm.body);
        });
    }

    fn visit_expr_for_loop_mut(&mut self, expr: &mut syn::ExprForLoop) {
        self.iterable(&mut expr.expr);
        self.visit_expr_mut(&mut expr.expr);
        self.scoped(|fold| {
            fold.bind(&expr.pat, None);
            fold.visit_block_mut(&mut expr.body);
        });
    }

    fn visit_expr_if_mut(&mut self, branch: &mut syn::ExprIf) {
        self.scoped(|fold| {
            fold.condition(&mut branch.cond);
            fold.visit_block_mut(&mut branch.then_branch);
        });
        if let Some((_, otherwise)) = &mut branch.else_branch {
            self.visit_expr_mut(otherwise);
        }
    }

    fn visit_expr_while_mut(&mut self, expr: &mut syn::ExprWhile) {
        self.scoped(|fold| {
            fold.condition(&mut expr.cond);
            fold.visit_block_mut(&mut expr.body);
        });
    }

    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        if !crate::drift::expr_attrs(expr).is_empty() {
            return;
        }
        syn::visit_mut::visit_expr_mut(self, expr);
        self.emptiness(expr);
    }
}

impl Fold {
    /// Visit a condition, binding the patterns of its `let` operands for the guarded block.
    fn condition(&mut self, cond: &mut syn::Expr) {
        match cond {
            syn::Expr::Let(binding) => {
                self.visit_expr_mut(&mut binding.expr);
                self.bind(&binding.pat, None);
            }
            syn::Expr::Binary(binary) if matches!(binary.op, syn::BinOp::And(_)) => {
                self.condition(&mut binary.left);
                self.condition(&mut binary.right);
            }
            other => self.visit_expr_mut(other),
        }
    }
}
