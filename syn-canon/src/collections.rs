//! Iteration, emptiness tests and string copies over bindings of proven standard collection
//! types.
//!
//! A binding declared as `Vec<T>`, `[T; N]`, `[T]`, `String` or `str`, behind at most one
//! reference, reaches the standard `iter`, `iter_mut`, `into_iter`, `len` and `is_empty` when no
//! trait in view declares those names. `for x in v.iter()` then walks what `for x in &v` walks,
//! and `v.len() == 0` holds exactly when `v.is_empty()` does. A `&str` binding reaches the
//! standard `to_string` and `to_owned`, and a `String` or `&String` binding also `clone` and
//! `as_str`, when no trait in view declares those names and the prelude traits keep their names.
//! Each copy then allocates the length of the text, so every spelling reads as `s.to_string()`.

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
    let named: BTreeSet<Kind> = [
        (Kind::Vec, "Vec"),
        (Kind::String, "String"),
        (Kind::Str, "str"),
    ]
    .into_iter()
    .filter(|(_, name)| !claims.glob && unclaimed(name))
    .map(|(kind, _)| kind)
    .collect();
    let mut fold = Fold {
        copies: named.contains(&Kind::String)
            && renamer.owned_methods()
            && renamer.clone_free()
            && ["ToString", "ToOwned", "Clone"]
                .iter()
                .all(|name| unclaimed(name)),
        accessors: renamer.accessor_free() && unclaimed("Iterator"),
        patterns: renamer.pattern_free(),
        named,
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    /// The kinds whose type names resolve to the standard `Vec`, `String` and `str`.
    named: BTreeSet<Kind>,
    /// Whether the standard copy methods answer to `to_string`, `to_owned`, `clone` and
    /// `as_str`, and `String` names the standard type.
    copies: bool,
    /// Whether the standard `get`, `first`, `as_bytes`, `as_slice`, `next` and `count` answer
    /// their calls and `Iterator` names the standard trait.
    accessors: bool,
    /// Whether the standard `str` pattern methods and the `push_str` and `push` of `String`
    /// answer their calls.
    patterns: bool,
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
                let kind = match (ident_name(&segment.ident).as_str(), &segment.arguments) {
                    ("Vec", syn::PathArguments::AngleBracketed(args)) if args.args.len() == 1 => {
                        Kind::Vec
                    }
                    ("String", syn::PathArguments::None) => Kind::String,
                    ("str", syn::PathArguments::None) => Kind::Str,
                    _ => return None,
                };
                if !self.named.contains(&kind) {
                    return None;
                }
                kind
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
        *expr = emptiness_test(receiver, empty);
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

    /// Read `s.to_owned()` and `String::from(s)` over a `&str` binding, and `s.to_owned()`,
    /// `s.clone()` and `String::from(s.as_str())` over a `String` or `&String` binding, as
    /// `s.to_string()`.
    fn copy(&self, expr: &mut syn::Expr) {
        if !self.copies {
            return;
        }
        let receiver = match expr {
            syn::Expr::MethodCall(call) if bare_call(call) => {
                let copies = matches!(
                    (
                        self.copied(&call.receiver),
                        ident_name(&call.method).as_str()
                    ),
                    (Some(Kind::Str), "to_owned") | (Some(Kind::String), "to_owned" | "clone")
                );
                if !copies {
                    return;
                }
                call.receiver.as_mut()
            }
            syn::Expr::Call(call)
                if call.attrs.is_empty() && call.args.len() == 1 && string_from(&call.func) =>
            {
                let text = &mut call.args[0];
                if self.copied(text) == Some(Kind::Str) {
                    text
                } else if let syn::Expr::MethodCall(inner) = text
                    && bare_call(inner)
                    && inner.method == "as_str"
                    && self.copied(&inner.receiver) == Some(Kind::String)
                {
                    inner.receiver.as_mut()
                } else {
                    return;
                }
            }
            _ => return,
        };
        let receiver = core::mem::replace(
            receiver,
            syn::Expr::Verbatim(proc_macro2::TokenStream::new()),
        );
        *expr = method_call(receiver, "to_string");
    }

    /// The kind of a binding whose copies share one reading, a `&str`, a `String` or a
    /// `&String`.
    fn copied(&self, expr: &syn::Expr) -> Option<Kind> {
        match self.binding(expr)? {
            Collection {
                kind: Kind::Str,
                access: Access::Shared,
            } => Some(Kind::Str),
            Collection {
                kind: Kind::String,
                access: Access::Owned | Access::Shared,
            } => Some(Kind::String),
            _ => None,
        }
    }

    /// Read a one-character string literal passed as the pattern of a standard `str` method
    /// over a `String` or `str` binding as its `char` literal, and `s.push_str` of one
    /// character over a `String` binding as `s.push` of it.
    fn pattern(&self, expr: &mut syn::Expr) {
        if !self.patterns {
            return;
        }
        let syn::Expr::MethodCall(call) = expr else {
            return;
        };
        let Some(collection) = self.binding(&call.receiver) else {
            return;
        };
        let method = ident_name(&call.method);
        let (index, arity) = match method.as_str() {
            "push_str" if collection.kind == Kind::String => (0, 1),
            "split" | "rsplit" | "split_once" | "rsplit_once" | "contains" | "starts_with"
            | "ends_with" | "find" | "rfind" | "matches" | "strip_prefix" | "strip_suffix"
            | "trim_matches" | "trim_start_matches" | "trim_end_matches" => (0, 1),
            "splitn" => (1, 2),
            "replace" => (0, 2),
            "replacen" => (0, 3),
            _ => return,
        };
        if !call.attrs.is_empty()
            || call.turbofish.is_some()
            || call.args.len() != arity
            || !matches!(collection.kind, Kind::String | Kind::Str)
        {
            return;
        }
        let Some(character) = single_character(&call.args[index]) else {
            return;
        };
        call.args[index] = syn::Expr::Lit(syn::ExprLit {
            attrs: Vec::new(),
            lit: syn::Lit::Char(syn::LitChar::new(character, proc_macro2::Span::call_site())),
        });
        if method == "push_str" {
            call.method = syn::Ident::new("push", call.method.span());
        }
    }

    /// Read the accessor synonyms of a proven collection binding as one form.
    fn accessor(&self, expr: &mut syn::Expr) {
        if !self.accessors {
            return;
        }
        let read = match expr {
            syn::Expr::Reference(reference) => self.view(reference),
            syn::Expr::Binary(binary) => self.empty_text(binary),
            syn::Expr::MethodCall(call) if bare_call(call) || call.method == "get" => {
                self.element(call)
            }
            _ => None,
        };
        if let Some(read) = read {
            *expr = read;
        }
    }

    /// Read `&s[..]` over a `String` and `&v[..]` over a `Vec` as `s.as_str()` and
    /// `v.as_slice()`, and `&*s` and `&*v` the same way over owned bindings, where the deref
    /// reaches the view.
    fn view(&self, reference: &syn::ExprReference) -> Option<syn::Expr> {
        if !reference.attrs.is_empty() || reference.mutability.is_some() {
            return None;
        }
        let (place, deref) = match reference.expr.as_ref() {
            syn::Expr::Index(index) if index.attrs.is_empty() && full_range(&index.index) => {
                (index.expr.as_ref(), false)
            }
            syn::Expr::Unary(unary)
                if unary.attrs.is_empty() && matches!(unary.op, syn::UnOp::Deref(_)) =>
            {
                (unary.expr.as_ref(), true)
            }
            _ => return None,
        };
        let collection = self.binding(place)?;
        if deref && collection.access != Access::Owned {
            return None;
        }
        let method = match collection.kind {
            Kind::String => "as_str",
            Kind::Vec => "as_slice",
            _ => return None,
        };
        Some(method_call(place.clone(), method))
    }

    /// Read `s == ""` and its mirror over a `String` or `str` binding as `s.is_empty()`, and
    /// `s != ""` and its mirror as `!s.is_empty()`.
    fn empty_text(&self, binary: &syn::ExprBinary) -> Option<syn::Expr> {
        let empty = match binary.op {
            syn::BinOp::Eq(_) => true,
            syn::BinOp::Ne(_) => false,
            _ => return None,
        };
        let text = |side: &syn::Expr| {
            self.binding(side)
                .is_some_and(|collection| matches!(collection.kind, Kind::String | Kind::Str))
        };
        let receiver = if text(&binary.left) && empty_literal(&binary.right) {
            binary.left.as_ref()
        } else if text(&binary.right) && empty_literal(&binary.left) {
            binary.right.as_ref()
        } else {
            return None;
        };
        Some(emptiness_test(receiver.clone(), empty))
    }

    /// Read `v.get(0)` and `v.iter().next()` over a `Vec`, array or slice binding as
    /// `v.first()`, `v.iter().count()` as `v.len()`, and `s.as_bytes().len()` over a `String`
    /// or `str` binding as `s.len()`.
    fn element(&self, call: &syn::ExprMethodCall) -> Option<syn::Expr> {
        let sequence = |expr: &syn::Expr| {
            self.binding(expr).is_some_and(|collection| {
                matches!(collection.kind, Kind::Vec | Kind::Array | Kind::Slice)
            })
        };
        let inner = match call.receiver.as_ref() {
            syn::Expr::MethodCall(inner) if bare_call(inner) => Some(inner),
            _ => None,
        };
        let (receiver, method) = match ident_name(&call.method).as_str() {
            "get"
                if call.attrs.is_empty()
                    && call.turbofish.is_none()
                    && call.args.len() == 1
                    && small_literal(&call.args[0]) == Some(0)
                    && sequence(&call.receiver) =>
            {
                (call.receiver.as_ref(), "first")
            }
            "next" | "count" => {
                let inner = inner.filter(|inner| inner.method == "iter")?;
                if !sequence(&inner.receiver) {
                    return None;
                }
                let method = if call.method == "next" {
                    "first"
                } else {
                    "len"
                };
                (inner.receiver.as_ref(), method)
            }
            "len" => {
                let inner = inner.filter(|inner| inner.method == "as_bytes")?;
                let text = self
                    .binding(&inner.receiver)
                    .is_some_and(|collection| matches!(collection.kind, Kind::String | Kind::Str));
                if !text {
                    return None;
                }
                (inner.receiver.as_ref(), "len")
            }
            _ => return None,
        };
        Some(method_call(receiver.clone(), method))
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

/// Whether `call` is an unattributed method call without arguments or turbofish.
fn bare_call(call: &syn::ExprMethodCall) -> bool {
    call.attrs.is_empty() && call.turbofish.is_none() && call.args.is_empty()
}

/// Whether `func` is the unattributed relative path `String::from`.
fn string_from(func: &syn::Expr) -> bool {
    let syn::Expr::Path(path) = func else {
        return false;
    };
    let [owner, name] = path.path.segments.iter().collect::<Vec<_>>()[..] else {
        return false;
    };
    path.attrs.is_empty()
        && path.qself.is_none()
        && path.path.leading_colon.is_none()
        && owner.arguments.is_none()
        && name.arguments.is_none()
        && owner.ident == "String"
        && name.ident == "from"
}

/// Whether `index` is the unattributed full range `..`.
fn full_range(index: &syn::Expr) -> bool {
    matches!(
        index,
        syn::Expr::Range(range)
            if range.attrs.is_empty()
                && range.start.is_none()
                && range.end.is_none()
                && matches!(range.limits, syn::RangeLimits::HalfOpen(_))
    )
}

/// Whether `expr` is the unattributed, unsuffixed empty string literal.
fn empty_literal(expr: &syn::Expr) -> bool {
    matches!(
        expr,
        syn::Expr::Lit(syn::ExprLit { attrs, lit: syn::Lit::Str(text) })
            if attrs.is_empty() && text.suffix().is_empty() && text.value().is_empty()
    )
}

/// The one character of an unattributed, unsuffixed string literal that holds exactly one.
fn single_character(expr: &syn::Expr) -> Option<char> {
    let syn::Expr::Lit(syn::ExprLit {
        attrs,
        lit: syn::Lit::Str(text),
    }) = expr
    else {
        return None;
    };
    if !attrs.is_empty() || !text.suffix().is_empty() {
        return None;
    }
    let value = text.value();
    let mut characters = value.chars();
    let character = characters.next()?;
    characters.next().is_none().then_some(character)
}

/// The call `receiver.method()`.
fn method_call(receiver: syn::Expr, method: &str) -> syn::Expr {
    syn::Expr::MethodCall(syn::ExprMethodCall {
        attrs: Vec::new(),
        receiver: Box::new(receiver),
        dot_token: syn::token::Dot::default(),
        method: syn::Ident::new(method, proc_macro2::Span::call_site()),
        turbofish: None,
        paren_token: syn::token::Paren::default(),
        args: syn::punctuated::Punctuated::new(),
    })
}

/// The test `receiver.is_empty()` when `empty`, else its negation.
fn emptiness_test(receiver: syn::Expr, empty: bool) -> syn::Expr {
    let test = method_call(receiver, "is_empty");
    if empty {
        return test;
    }
    syn::Expr::Unary(syn::ExprUnary {
        attrs: Vec::new(),
        op: syn::UnOp::Not(syn::token::Not::default()),
        expr: Box::new(test),
    })
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
        self.accessor(expr);
        self.pattern(expr);
        self.emptiness(expr);
        self.copy(expr);
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
