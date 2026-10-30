//! Predicates and conversions of bindings declared as the prelude `Option` or `Result`.
//!
//! A binding declared as `Option<T>` or `Result<T, E>`, behind at most one reference, answers
//! `is_some`, `is_none`, `is_ok` and `is_err` with its inherent methods, which no trait method
//! can replace, so each reads as the `match` it performs. `o == None` holds exactly when `o` is
//! `None`, since the derived comparison never calls `T::eq` against `None`. `unwrap_or` of an
//! owned `Option` with a literal or primitive default, the `match` that rebuilds an owned
//! `Option` and `std::mem::replace(o, None)` over a `&mut Option` read as what they compute.
//! The prelude variants spelled through `Option::` and `Result::` read as their bare names.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use syn::visit_mut::VisitMut;

use crate::alpha::Renamer;
use crate::scope::ident_name;

/// The prelude names the readings rely on.
const PRELUDE: [&str; 6] = ["Option", "Result", "Some", "None", "Ok", "Err"];

/// Read the `Option` and `Result` predicates and conversions of the body `block` of a function
/// with the signature `sig`, nested items aside.
pub(crate) fn normalize(renamer: &Renamer<'_>, sig: &syn::Signature, block: &mut syn::Block) {
    let mut claims = Claims::default();
    syn::visit::Visit::visit_signature(&mut claims, sig);
    syn::visit::Visit::visit_block(&mut claims, block);
    if claims.open
        || !renamer.prelude_names(&PRELUDE)
        || PRELUDE.iter().any(|name| claims.names.contains(*name))
    {
        return;
    }
    let mut fold = Fold {
        renamer,
        scopes: alloc::vec![BTreeMap::new()],
    };
    for input in &sig.inputs {
        if let syn::FnArg::Typed(typed) = input {
            let declared = fold.declared(&typed.ty);
            fold.bind(&typed.pat, declared);
        }
    }
    fold.visit_block_mut(block);
}

/// The names an item, import or generic parameter of the function declares, and whether a
/// glob import may declare any name.
#[derive(Default)]
struct Claims {
    names: BTreeSet<String>,
    open: bool,
}

impl syn::visit::Visit<'_> for Claims {
    fn visit_item(&mut self, item: &syn::Item) {
        if let Some((_, ident)) = crate::alpha::item_binding(item) {
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
        self.open = true;
    }

    fn visit_type_param(&mut self, param: &syn::TypeParam) {
        self.names.insert(ident_name(&param.ident));
        syn::visit::visit_type_param(self, param);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Option,
    Result,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Owned,
    Shared,
    Mut,
}

/// What a declared type proves about a binding.
#[derive(Clone, Copy)]
enum Declared {
    Variant(Kind, Access),
    Primitive,
}

struct Fold<'a, 'env> {
    renamer: &'a Renamer<'env>,
    /// The bindings in view, with what their declared type proves.
    scopes: Vec<BTreeMap<String, Option<Declared>>>,
}

impl Fold<'_, '_> {
    /// What the declared type `ty` proves, behind at most one reference.
    fn declared(&self, ty: &syn::Type) -> Option<Declared> {
        if self.renamer.param_prim(ty).is_some() {
            return Some(Declared::Primitive);
        }
        let (access, ty) = match ty {
            syn::Type::Reference(reference) if reference.mutability.is_some() => {
                (Access::Mut, reference.elem.as_ref())
            }
            syn::Type::Reference(reference) => (Access::Shared, reference.elem.as_ref()),
            ty => (Access::Owned, ty),
        };
        let syn::Type::Path(path) = ty else {
            return None;
        };
        if path.qself.is_some() || path.path.leading_colon.is_some() {
            return None;
        }
        let [segment] = path.path.segments.iter().collect::<Vec<_>>()[..] else {
            return None;
        };
        let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
            return None;
        };
        let kind = match (ident_name(&segment.ident).as_str(), args.args.len()) {
            ("Option", 1) => Kind::Option,
            ("Result", 2) => Kind::Result,
            _ => return None,
        };
        Some(Declared::Variant(kind, access))
    }

    /// Bind the names of `pat`, a lone identifier to `declared` and every other name to
    /// `None`.
    fn bind(&mut self, pat: &syn::Pat, declared: Option<Declared>) {
        struct Names<'s>(&'s mut BTreeMap<String, Option<Declared>>, Option<Declared>);
        impl syn::visit::Visit<'_> for Names<'_> {
            fn visit_pat_ident(&mut self, ident: &syn::PatIdent) {
                let declared = self
                    .1
                    .filter(|_| ident.by_ref.is_none() && ident.subpat.is_none());
                self.0.insert(ident_name(&ident.ident), declared);
                syn::visit::visit_pat_ident(self, ident);
            }

            fn visit_expr(&mut self, _: &syn::Expr) {}
        }
        let declared = declared.filter(|_| matches!(pat, syn::Pat::Ident(_)));
        let scope = self.scopes.last_mut().expect("a scope");
        syn::visit::Visit::visit_pat(&mut Names(scope, declared), pat);
    }

    /// What the declaration of the single-identifier path `expr` proves.
    fn binding(&self, expr: &syn::Expr) -> Option<Declared> {
        let syn::Expr::Path(path) = expr else {
            return None;
        };
        if !path.attrs.is_empty() || path.qself.is_some() {
            return None;
        }
        let name = ident_name(path.path.get_ident()?);
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&name))
            .copied()
            .flatten()
    }

    /// The kind and access of a binding declared as `Option` or `Result`.
    fn variant(&self, expr: &syn::Expr) -> Option<(Kind, Access)> {
        match self.binding(expr)? {
            Declared::Variant(kind, access) => Some((kind, access)),
            Declared::Primitive => None,
        }
    }

    /// Read a predicate call or a comparison with `None` as its `match`.
    fn predicate(&self, expr: &mut syn::Expr) {
        let (receiver, kind, holds) = match expr {
            syn::Expr::MethodCall(call)
                if call.attrs.is_empty() && call.turbofish.is_none() && call.args.is_empty() =>
            {
                let Some((kind, _)) = self.variant(&call.receiver) else {
                    return;
                };
                let holds = match (kind, ident_name(&call.method).as_str()) {
                    (Kind::Option, "is_some") | (Kind::Result, "is_ok") => true,
                    (Kind::Option, "is_none") | (Kind::Result, "is_err") => false,
                    _ => return,
                };
                (call.receiver.as_mut(), kind, holds)
            }
            syn::Expr::Binary(binary)
                if binary.attrs.is_empty()
                    && matches!(binary.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_)) =>
            {
                let equal = matches!(binary.op, syn::BinOp::Eq(_));
                let receiver = if self.is_none(&binary.right)
                    && self.variant(&binary.left) == Some((Kind::Option, Access::Owned))
                {
                    binary.left.as_mut()
                } else if self.is_none(&binary.left)
                    && self.variant(&binary.right) == Some((Kind::Option, Access::Owned))
                {
                    binary.right.as_mut()
                } else {
                    return;
                };
                (receiver, Kind::Option, !equal)
            }
            _ => return,
        };
        let scrutinee = core::mem::replace(receiver, placeholder());
        let head = match kind {
            Kind::Option => "Some",
            Kind::Result => "Ok",
        };
        *expr = syn::Expr::Match(syn::ExprMatch {
            attrs: Vec::new(),
            match_token: syn::token::Match::default(),
            expr: Box::new(scrutinee),
            brace_token: syn::token::Brace::default(),
            arms: alloc::vec![
                arm(variant_pattern(head, wild()), bool_expr(holds)),
                arm(wild(), bool_expr(!holds)),
            ],
        });
    }

    /// Whether `expr` is the bare prelude `None`.
    fn is_none(&self, expr: &syn::Expr) -> bool {
        self.renamer.prelude_variant(expr, &["None"])
    }

    /// Read `o.unwrap_or(c)` and `o.unwrap_or_else(|| c)` over an owned `Option` with a literal
    /// or primitive default as `match o { Some(value) => value, None => c }`.
    fn default(&self, expr: &mut syn::Expr) {
        let syn::Expr::MethodCall(call) = expr else {
            return;
        };
        if !call.attrs.is_empty()
            || call.turbofish.is_some()
            || call.args.len() != 1
            || self.variant(&call.receiver) != Some((Kind::Option, Access::Owned))
        {
            return;
        }
        let fallback = match (ident_name(&call.method).as_str(), &call.args[0]) {
            ("unwrap_or", fallback) => fallback,
            ("unwrap_or_else", syn::Expr::Closure(closure))
                if closure.attrs.is_empty()
                    && closure.inputs.is_empty()
                    && matches!(closure.output, syn::ReturnType::Default) =>
            {
                closure.body.as_ref()
            }
            _ => return,
        };
        let plain = match fallback {
            syn::Expr::Lit(lit) => lit.attrs.is_empty(),
            path @ syn::Expr::Path(_) => matches!(self.binding(path), Some(Declared::Primitive)),
            _ => false,
        };
        if !plain {
            return;
        }
        let fallback = fallback.clone();
        let scrutinee = core::mem::replace(call.receiver.as_mut(), placeholder());
        let value = syn::Ident::new("value", proc_macro2::Span::call_site());
        *expr = syn::Expr::Match(syn::ExprMatch {
            attrs: Vec::new(),
            match_token: syn::token::Match::default(),
            expr: Box::new(scrutinee),
            brace_token: syn::token::Brace::default(),
            arms: alloc::vec![
                arm(
                    variant_pattern("Some", ident_pat(value.clone())),
                    path_expr(value)
                ),
                arm(ident_pat(name("None")), fallback),
            ],
        });
    }

    /// Read `match o { Some(v) => Some(v), None => None }` over an owned `Option`, in either
    /// arm order and with `_` for `None`, as `o`.
    fn identity(&self, expr: &mut syn::Expr) {
        let syn::Expr::Match(expr_match) = expr else {
            return;
        };
        if !expr_match.attrs.is_empty()
            || self.variant(&expr_match.expr) != Some((Kind::Option, Access::Owned))
        {
            return;
        }
        let [first, second] = expr_match.arms.as_slice() else {
            return;
        };
        let order = (self.rebuilds_some(first) && self.passes_none(second, false))
            || (self.passes_none(first, true) && self.rebuilds_some(second));
        if order {
            *expr = core::mem::replace(expr_match.expr.as_mut(), placeholder());
        }
    }

    /// Whether `arm` is `Some(v) => Some(v)` for a plain binding `v`.
    fn rebuilds_some(&self, arm: &syn::Arm) -> bool {
        let syn::Pat::TupleStruct(pattern) = &arm.pat else {
            return false;
        };
        let [syn::Pat::Ident(binding)] = pattern.elems.iter().collect::<Vec<_>>()[..] else {
            return false;
        };
        let syn::Expr::Call(call) = arm.body.as_ref() else {
            return false;
        };
        let rebuilt = matches!(
            &call.args.iter().collect::<Vec<_>>()[..],
            [syn::Expr::Path(path)]
                if path.attrs.is_empty()
                    && path.qself.is_none()
                    && path.path.get_ident().is_some_and(|ident| ident_name(ident) == ident_name(&binding.ident))
        );
        arm.attrs.is_empty()
            && pattern.attrs.is_empty()
            && pattern.qself.is_none()
            && pattern.path.is_ident("Some")
            && binding.attrs.is_empty()
            && binding.by_ref.is_none()
            && binding.mutability.is_none()
            && binding.subpat.is_none()
            && call.attrs.is_empty()
            && self.renamer.prelude_variant(&call.func, &["Some"])
            && rebuilt
    }

    /// Whether `arm` is `None => None`, or `_ => None` when it is not the `first` arm.
    fn passes_none(&self, arm: &syn::Arm, first: bool) -> bool {
        let pattern = match &arm.pat {
            syn::Pat::Ident(ident) => {
                ident.attrs.is_empty()
                    && ident.by_ref.is_none()
                    && ident.mutability.is_none()
                    && ident.subpat.is_none()
                    && ident.ident == "None"
            }
            syn::Pat::Wild(wild) => !first && wild.attrs.is_empty(),
            _ => false,
        };
        arm.attrs.is_empty() && pattern && self.is_none(&arm.body)
    }

    /// Read `std::mem::replace(o, None)` over a `&mut Option` as `o.take()`.
    fn take(&self, expr: &mut syn::Expr) {
        let syn::Expr::Call(call) = expr else {
            return;
        };
        let [target, replacement] = call.args.iter().collect::<Vec<_>>()[..] else {
            return;
        };
        if !call.attrs.is_empty()
            || !self.renamer.standard_function(&call.func, "mem", "replace")
            || !self.is_none(replacement)
            || self.variant(target) != Some((Kind::Option, Access::Mut))
        {
            return;
        }
        let target = target.clone();
        *expr = syn::Expr::MethodCall(syn::ExprMethodCall {
            attrs: Vec::new(),
            receiver: Box::new(target),
            dot_token: syn::token::Dot::default(),
            method: name("take"),
            turbofish: None,
            paren_token: syn::token::Paren::default(),
            args: syn::punctuated::Punctuated::new(),
        });
    }

    fn scoped(&mut self, visit: impl FnOnce(&mut Self)) {
        self.scopes.push(BTreeMap::new());
        visit(self);
        self.scopes.pop();
    }
}

/// Spell `Option::Some`, `Option::None`, `Result::Ok` and `Result::Err` as the bare variant.
fn bare_variant(path: &mut syn::Path) {
    let [owner, variant] = path.segments.iter().collect::<Vec<_>>()[..] else {
        return;
    };
    let qualified = matches!(
        (
            ident_name(&owner.ident).as_str(),
            ident_name(&variant.ident).as_str()
        ),
        ("Option", "Some" | "None") | ("Result", "Ok" | "Err")
    );
    if path.leading_colon.is_none()
        && qualified
        && owner.arguments.is_none()
        && variant.arguments.is_none()
    {
        path.segments = path.segments.iter().skip(1).cloned().collect();
    }
}

impl VisitMut for Fold<'_, '_> {
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
        self.visit_pat_mut(&mut local.pat);
        let (pat, declared) = match &local.pat {
            syn::Pat::Type(typed) if local.attrs.is_empty() => {
                (typed.pat.as_ref(), self.declared(&typed.ty))
            }
            pat => (pat, None),
        };
        self.bind(pat, declared);
    }

    fn visit_expr_closure_mut(&mut self, closure: &mut syn::ExprClosure) {
        self.scoped(|fold| {
            for input in &closure.inputs {
                let (pat, declared) = match input {
                    syn::Pat::Type(typed) => (typed.pat.as_ref(), fold.declared(&typed.ty)),
                    pat => (pat, None),
                };
                fold.bind(pat, declared);
            }
            fold.visit_expr_mut(&mut closure.body);
        });
    }

    fn visit_arm_mut(&mut self, arm: &mut syn::Arm) {
        self.scoped(|fold| {
            fold.visit_pat_mut(&mut arm.pat);
            fold.bind(&arm.pat, None);
            if let syn::Pat::Guard(guard) = &mut arm.pat {
                fold.visit_expr_mut(&mut guard.guard);
            }
            fold.visit_expr_mut(&mut arm.body);
        });
    }

    fn visit_expr_for_loop_mut(&mut self, expr: &mut syn::ExprForLoop) {
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

    fn visit_pat_mut(&mut self, pat: &mut syn::Pat) {
        match pat {
            syn::Pat::TupleStruct(pattern) if pattern.qself.is_none() => {
                bare_variant(&mut pattern.path);
            }
            syn::Pat::Path(path) if path.qself.is_none() => {
                bare_variant(&mut path.path);
                if let Some(ident) = path.path.get_ident().cloned() {
                    *pat = ident_pat(ident);
                    return;
                }
            }
            _ => {}
        }
        syn::visit_mut::visit_pat_mut(self, pat);
    }

    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        if !crate::drift::expr_attrs(expr).is_empty() {
            return;
        }
        if let syn::Expr::Path(path) = expr
            && path.qself.is_none()
        {
            bare_variant(&mut path.path);
        }
        syn::visit_mut::visit_expr_mut(self, expr);
        self.predicate(expr);
        self.default(expr);
        self.identity(expr);
        self.take(expr);
    }
}

impl Fold<'_, '_> {
    /// Visit a condition, binding the patterns of its `let` operands for the guarded block.
    fn condition(&mut self, cond: &mut syn::Expr) {
        match cond {
            syn::Expr::Let(binding) => {
                self.visit_expr_mut(&mut binding.expr);
                self.visit_pat_mut(&mut binding.pat);
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

fn placeholder() -> syn::Expr {
    syn::Expr::Verbatim(proc_macro2::TokenStream::new())
}

fn name(text: &str) -> syn::Ident {
    syn::Ident::new(text, proc_macro2::Span::call_site())
}

fn wild() -> syn::Pat {
    syn::Pat::Wild(syn::PatWild {
        attrs: Vec::new(),
        underscore_token: syn::token::Underscore::default(),
    })
}

fn ident_pat(ident: syn::Ident) -> syn::Pat {
    syn::Pat::Ident(syn::PatIdent {
        attrs: Vec::new(),
        by_ref: None,
        mutability: None,
        ident,
        subpat: None,
    })
}

fn variant_pattern(variant: &str, inner: syn::Pat) -> syn::Pat {
    syn::Pat::TupleStruct(syn::PatTupleStruct {
        attrs: Vec::new(),
        qself: None,
        path: syn::Path::from(name(variant)),
        paren_token: syn::token::Paren::default(),
        elems: core::iter::once(inner).collect(),
    })
}

fn path_expr(ident: syn::Ident) -> syn::Expr {
    syn::Expr::Path(syn::ExprPath {
        attrs: Vec::new(),
        qself: None,
        path: syn::Path::from(ident),
    })
}

fn bool_expr(value: bool) -> syn::Expr {
    syn::Expr::Lit(syn::ExprLit {
        attrs: Vec::new(),
        lit: syn::Lit::Bool(syn::LitBool::new(value, proc_macro2::Span::call_site())),
    })
}

fn arm(pat: syn::Pat, body: syn::Expr) -> syn::Arm {
    syn::Arm {
        attrs: Vec::new(),
        pat,
        fat_arrow_token: syn::token::FatArrow::default(),
        body: Box::new(body),
        comma: None,
    }
}
