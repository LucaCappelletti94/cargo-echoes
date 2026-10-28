//! `if let` over a place and the arms of a `match` under resolution.
//!
//! A place scrutinee creates no temporary, so `if let P = e { a } else { b }` and
//! `match e { P => { a } _ => { b } }` drop the same values at the same points. With the prelude
//! `Option` in view, `None` after `Some(p)` for an irrefutable `p` matches exactly what `_` does
//! there, and `None` binds nothing, so equal arms join around it. A `match` runs the one arm
//! whose pattern matches, so arms of distinct literals or of distinct variants of one enum read
//! the same in any order.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use syn::visit_mut::VisitMut;

use crate::alpha::Renamer;
use crate::scope::{Origin, ident_name};

/// The prelude variants of `Option` and `Result`, each with its declaration position.
const PRELUDE_VARIANTS: [(&str, usize); 4] = [("None", 0), ("Some", 1), ("Ok", 0), ("Err", 1)];

/// Read the `if let` expressions and `match` arms of the body `block`, nested items aside.
pub(crate) fn normalize(renamer: &Renamer<'_>, block: &mut syn::Block) {
    let mut fold = Fold {
        renamer,
        scopes: Vec::new(),
    };
    fold.visit_block_mut(block);
}

/// The names a block, closure, arm or condition declares.
#[derive(Default)]
struct Scope {
    /// Value names, a binding mapped to `true` and an item or import to `false`.
    values: BTreeMap<String, bool>,
    /// Type names, an enum mapped to its variants.
    types: BTreeMap<String, Option<Vec<String>>>,
    /// A glob import or a macro may declare any name here.
    open: bool,
}

impl Scope {
    fn claim(&mut self, name: String) {
        self.values.insert(name.clone(), false);
        self.types.insert(name, None);
    }

    /// Claim the names a use tree binds, `self` binding the module it names.
    fn import(&mut self, tree: &syn::UseTree, parent: Option<&syn::Ident>) {
        match tree {
            syn::UseTree::Path(path) => self.import(&path.tree, Some(&path.ident)),
            syn::UseTree::Name(name) => self.claim(ident_name(
                parent
                    .filter(|_| name.ident == "self")
                    .unwrap_or(&name.ident),
            )),
            syn::UseTree::Rename(rename) => self.claim(ident_name(&rename.rename)),
            syn::UseTree::Glob(_) => self.open = true,
            syn::UseTree::Group(group) => {
                for tree in &group.items {
                    self.import(tree, parent);
                }
            }
        }
    }
}

/// A literal value or a variant position. Compiling arms of one `match` hold keys of one type,
/// so keys that differ name disjoint patterns.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Int(i128),
    Bool(bool),
    Char(char),
    Byte(u8),
    Str(String),
    Bytes(Vec<u8>),
    Variant(usize),
}

struct Fold<'a, 'env> {
    renamer: &'a Renamer<'env>,
    scopes: Vec<Scope>,
}

impl Fold<'_, '_> {
    /// The scope of the items and imports of `block`, which are visible in all of it.
    fn declarations(&self, block: &syn::Block) -> Scope {
        let mut scope = Scope {
            open: crate::tails::blocked(self.renamer, block),
            ..Scope::default()
        };
        for stmt in &block.stmts {
            let syn::Stmt::Item(item) = stmt else {
                continue;
            };
            match item {
                syn::Item::Use(import) => scope.import(&import.tree, None),
                syn::Item::Enum(item) => {
                    let variants = item
                        .variants
                        .iter()
                        .map(|variant| ident_name(&variant.ident))
                        .collect();
                    scope.types.insert(ident_name(&item.ident), Some(variants));
                }
                syn::Item::Struct(item) if !matches!(item.fields, syn::Fields::Named(_)) => {
                    scope.claim(ident_name(&item.ident));
                }
                syn::Item::ExternCrate(item) => {
                    let ident = item.rename.as_ref().map_or(&item.ident, |(_, alias)| alias);
                    scope.types.insert(ident_name(ident), None);
                }
                other => {
                    if let Some((ns, ident)) = crate::alpha::item_binding(other) {
                        if ns == crate::scope::Ns::Value {
                            scope.values.insert(ident_name(ident), false);
                        } else {
                            scope.types.insert(ident_name(ident), None);
                        }
                    }
                }
            }
        }
        scope
    }

    /// Bind the names `pat` introduces in the innermost scope.
    fn bind(&mut self, pat: &syn::Pat) {
        struct Names<'s>(&'s mut Scope);
        impl syn::visit::Visit<'_> for Names<'_> {
            fn visit_pat_ident(&mut self, ident: &syn::PatIdent) {
                if !crate::alpha::is_unit_path(ident) {
                    self.0.values.insert(ident_name(&ident.ident), true);
                }
                syn::visit::visit_pat_ident(self, ident);
            }

            fn visit_expr(&mut self, _: &syn::Expr) {}
        }
        let scope = self.scopes.last_mut().expect("a scope");
        syn::visit::Visit::visit_pat(&mut Names(scope), pat);
    }

    /// Whether `name` resolves to a binding, `false` for an item, an import or an open scope,
    /// and `None` when nothing in view declares it.
    fn value(&self, name: &str) -> Option<bool> {
        for scope in self.scopes.iter().rev() {
            if let Some(&binding) = scope.values.get(name) {
                return Some(binding);
            }
            if scope.open {
                return Some(false);
            }
        }
        self.renamer
            .value_origin(name)
            .map(|origin| matches!(origin, Origin::Parameter | Origin::Let | Origin::Closure))
    }

    /// The declaration position of `variant` in the enum `name` resolves to.
    fn variant(&self, name: &str, variant: &str) -> Option<usize> {
        let position = |variants: &[String]| variants.iter().position(|known| known == variant);
        for scope in self.scopes.iter().rev() {
            if let Some(entry) = scope.types.get(name) {
                return entry.as_deref().and_then(position);
            }
            if scope.open {
                return None;
            }
        }
        position(self.renamer.enum_variants(name)?)
    }

    /// Whether the prelude variants answer to their bare names here.
    fn prelude(&self) -> bool {
        self.renamer
            .prelude_names(&PRELUDE_VARIANTS.map(|(name, _)| name))
            && self.scopes.iter().all(|scope| {
                !scope.open
                    && PRELUDE_VARIANTS.iter().all(|(name, _)| {
                        !scope.values.contains_key(*name) && !scope.types.contains_key(*name)
                    })
            })
    }

    /// Whether `expr` names a parameter, closure input or local binding.
    fn place(&self, expr: &syn::Expr) -> bool {
        matches!(
            expr,
            syn::Expr::Path(path)
                if path.attrs.is_empty()
                    && path.qself.is_none()
                    && path.path.get_ident().is_some_and(|ident| self.value(&ident_name(ident)) == Some(true))
        )
    }

    /// Whether `pat` matches every value of its type: a wildcard, a fresh binding, or a tuple or
    /// reference of such patterns.
    fn irrefutable(&self, pat: &syn::Pat) -> bool {
        match pat {
            syn::Pat::Wild(wild) => wild.attrs.is_empty(),
            syn::Pat::Rest(rest) => rest.attrs.is_empty(),
            syn::Pat::Ident(ident) => {
                ident.attrs.is_empty()
                    && !crate::alpha::is_unit_path(ident)
                    && self.value(&ident_name(&ident.ident)) != Some(false)
                    && ident
                        .subpat
                        .as_ref()
                        .is_none_or(|(_, subpat)| self.irrefutable(subpat))
            }
            syn::Pat::Tuple(tuple) => {
                tuple.attrs.is_empty() && tuple.elems.iter().all(|elem| self.irrefutable(elem))
            }
            syn::Pat::Reference(reference) => {
                reference.attrs.is_empty() && self.irrefutable(&reference.pat)
            }
            _ => false,
        }
    }

    /// Read `if let P = e { a } else { b }` over a place `e` as `match e { P => { a } _ => b }`,
    /// the missing `else` as the unit block.
    fn if_let(&self, expr: &mut syn::Expr) {
        let syn::Expr::If(branch) = expr else {
            return;
        };
        let syn::Expr::Let(binding) = branch.cond.as_mut() else {
            return;
        };
        if !binding.attrs.is_empty() || !self.place(&binding.expr) {
            return;
        }
        let scrutinee = core::mem::replace(&mut binding.expr, Box::new(placeholder()));
        let pat = core::mem::replace(
            binding.pat.as_mut(),
            syn::Pat::Verbatim(proc_macro2::TokenStream::new()),
        );
        let then = block_expr(core::mem::take(&mut branch.then_branch.stmts));
        let otherwise = branch
            .else_branch
            .take()
            .map_or_else(|| block_expr(Vec::new()), |(_, otherwise)| *otherwise);
        *expr = syn::Expr::Match(syn::ExprMatch {
            attrs: Vec::new(),
            match_token: syn::token::Match::default(),
            expr: scrutinee,
            brace_token: syn::token::Brace::default(),
            arms: alloc::vec![arm(pat, then), arm(wildcard(), otherwise)],
        });
    }

    /// Apply the arm readings to an unattributed `match`.
    fn arms(&self, expr_match: &mut syn::ExprMatch) {
        let prelude = self.prelude();
        if prelude {
            self.catch_none(expr_match);
        }
        self.order(expr_match, prelude);
        crate::drift::join_equal_arms(expr_match, &|ident| prelude && bare_none(ident));
    }

    /// Read the two arms `Some(p)` and `None` with an irrefutable `p`, in either order, as
    /// `Some(p)` followed by `_`.
    fn catch_none(&self, expr_match: &mut syn::ExprMatch) {
        let [first, second] = expr_match.arms.as_slice() else {
            return;
        };
        let none = if self.some_binding(first) && none_arm(second) {
            1
        } else if none_arm(first) && self.some_binding(second) {
            0
        } else {
            return;
        };
        let mut arm = expr_match.arms.remove(none);
        arm.pat = wildcard();
        expr_match.arms.push(arm);
    }

    /// Whether `arm` is an unattributed `Some(p)` arm with an irrefutable `p`.
    fn some_binding(&self, arm: &syn::Arm) -> bool {
        arm.attrs.is_empty()
            && matches!(
                &arm.pat,
                syn::Pat::TupleStruct(tuple)
                    if tuple.attrs.is_empty()
                        && tuple.qself.is_none()
                        && tuple.path.is_ident("Some")
                        && tuple.elems.len() == 1
                        && self.irrefutable(&tuple.elems[0])
            )
    }

    /// Sort each run of adjacent arms whose keys are pairwise distinct, up to the first arm
    /// that matches every value.
    fn order(&self, expr_match: &mut syn::ExprMatch, prelude: bool) {
        let arms = core::mem::take(&mut expr_match.arms);
        let mut ordered = Vec::with_capacity(arms.len());
        let mut run: Vec<(Key, syn::Arm)> = Vec::new();
        let mut seen = BTreeSet::new();
        let mut sealed = false;
        for arm in arms {
            if let Some(keys) = self.keys(&arm, prelude).filter(|_| !sealed) {
                if keys.iter().any(|key| seen.contains(key)) {
                    flush(&mut run, &mut ordered);
                    seen.clear();
                }
                let first = keys.iter().min().expect("a pattern has a key").clone();
                seen.extend(keys);
                run.push((first, arm));
            } else {
                flush(&mut run, &mut ordered);
                seen.clear();
                sealed |= self.irrefutable(&arm.pat);
                ordered.push(arm);
            }
        }
        flush(&mut run, &mut ordered);
        expr_match.arms = ordered;
    }

    /// The keys of an unattributed arm whose alternatives are all literals or variants.
    fn keys(&self, arm: &syn::Arm, prelude: bool) -> Option<Vec<Key>> {
        if !arm.attrs.is_empty() {
            return None;
        }
        match &arm.pat {
            syn::Pat::Or(or) => or.cases.iter().map(|pat| self.key(pat, prelude)).collect(),
            single => self.key(single, prelude).map(|key| alloc::vec![key]),
        }
    }

    fn key(&self, pat: &syn::Pat, prelude: bool) -> Option<Key> {
        let path = match pat {
            syn::Pat::Lit(lit) => return literal_key(&lit.lit),
            syn::Pat::Ident(ident) if prelude && bare_none(ident) => return Some(Key::Variant(0)),
            syn::Pat::Path(path) if path.qself.is_none() => &path.path,
            syn::Pat::TupleStruct(tuple) if tuple.qself.is_none() => &tuple.path,
            syn::Pat::Struct(pat) if pat.qself.is_none() => &pat.path,
            _ => return None,
        };
        self.variant_key(path, prelude)
    }

    /// The position of the variant `path` names: a prelude variant by its bare name, or `E::V`
    /// for an enum `E` declared in view.
    fn variant_key(&self, path: &syn::Path, prelude: bool) -> Option<Key> {
        if path.leading_colon.is_some()
            || path
                .segments
                .iter()
                .any(|segment| !matches!(segment.arguments, syn::PathArguments::None))
        {
            return None;
        }
        let names: Vec<String> = path
            .segments
            .iter()
            .map(|segment| ident_name(&segment.ident))
            .collect();
        let position = match names.as_slice() {
            [variant] if prelude => PRELUDE_VARIANTS
                .iter()
                .find(|(name, _)| name == variant)
                .map(|&(_, position)| position),
            [owner, variant] => self.variant(owner, variant),
            _ => None,
        };
        position.map(Key::Variant)
    }
}

impl VisitMut for Fold<'_, '_> {
    fn visit_item_mut(&mut self, _: &mut syn::Item) {}

    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        let scope = self.declarations(block);
        self.scopes.push(scope);
        for stmt in &mut block.stmts {
            self.visit_stmt_mut(stmt);
        }
        self.scopes.pop();
    }

    fn visit_local_mut(&mut self, local: &mut syn::Local) {
        if local.attrs.is_empty()
            && let Some(init) = &mut local.init
        {
            self.visit_expr_mut(&mut init.expr);
            if let Some((_, diverge)) = &mut init.diverge {
                self.visit_expr_mut(diverge);
            }
        }
        self.bind(&local.pat);
    }

    fn visit_expr_closure_mut(&mut self, closure: &mut syn::ExprClosure) {
        self.scopes.push(Scope::default());
        for input in &closure.inputs {
            self.bind(input);
        }
        self.visit_expr_mut(&mut closure.body);
        self.scopes.pop();
    }

    fn visit_arm_mut(&mut self, arm: &mut syn::Arm) {
        if !arm.attrs.is_empty() {
            return;
        }
        self.scopes.push(Scope::default());
        self.bind(&arm.pat);
        if let syn::Pat::Guard(guard) = &mut arm.pat {
            self.visit_expr_mut(&mut guard.guard);
        }
        self.visit_expr_mut(&mut arm.body);
        self.scopes.pop();
    }

    fn visit_expr_if_mut(&mut self, branch: &mut syn::ExprIf) {
        self.scopes.push(Scope::default());
        self.condition(&mut branch.cond);
        self.visit_block_mut(&mut branch.then_branch);
        self.scopes.pop();
        if let Some((_, otherwise)) = &mut branch.else_branch {
            self.visit_expr_mut(otherwise);
        }
    }

    fn visit_expr_while_mut(&mut self, expr: &mut syn::ExprWhile) {
        self.scopes.push(Scope::default());
        self.condition(&mut expr.cond);
        self.visit_block_mut(&mut expr.body);
        self.scopes.pop();
    }

    fn visit_expr_for_loop_mut(&mut self, expr: &mut syn::ExprForLoop) {
        self.visit_expr_mut(&mut expr.expr);
        self.scopes.push(Scope::default());
        self.bind(&expr.pat);
        self.visit_block_mut(&mut expr.body);
        self.scopes.pop();
    }

    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        if !crate::drift::expr_attrs(expr).is_empty() {
            return;
        }
        self.if_let(expr);
        syn::visit_mut::visit_expr_mut(self, expr);
        if let syn::Expr::Match(expr_match) = expr {
            self.arms(expr_match);
        }
    }
}

impl Fold<'_, '_> {
    /// Visit a condition, binding the patterns of its `let` operands for the guarded block.
    fn condition(&mut self, cond: &mut syn::Expr) {
        match cond {
            syn::Expr::Let(binding) if binding.attrs.is_empty() => {
                self.visit_expr_mut(&mut binding.expr);
                self.bind(&binding.pat);
            }
            syn::Expr::Binary(binary)
                if binary.attrs.is_empty() && matches!(binary.op, syn::BinOp::And(_)) =>
            {
                self.condition(&mut binary.left);
                self.condition(&mut binary.right);
            }
            other => self.visit_expr_mut(other),
        }
    }
}

/// Move a run of pairwise disjoint arms to `ordered` by their least key.
fn flush(run: &mut Vec<(Key, syn::Arm)>, ordered: &mut Vec<syn::Arm>) {
    run.sort_by(|left, right| left.0.cmp(&right.0));
    ordered.extend(run.drain(..).map(|(_, arm)| arm));
}

fn literal_key(lit: &syn::Lit) -> Option<Key> {
    Some(match lit {
        syn::Lit::Int(int) => Key::Int(int.base10_digits().parse().ok()?),
        syn::Lit::Bool(value) => Key::Bool(value.value),
        syn::Lit::Char(value) => Key::Char(value.value()),
        syn::Lit::Byte(value) => Key::Byte(value.value()),
        syn::Lit::Str(value) => Key::Str(value.value()),
        syn::Lit::ByteStr(value) => Key::Bytes(value.value()),
        _ => return None,
    })
}

/// Whether `ident` is the unattributed bare pattern `None`.
fn bare_none(ident: &syn::PatIdent) -> bool {
    ident.attrs.is_empty()
        && ident.by_ref.is_none()
        && ident.mutability.is_none()
        && ident.subpat.is_none()
        && ident.ident == "None"
}

/// Whether `arm` is the unattributed arm `None`.
fn none_arm(arm: &syn::Arm) -> bool {
    arm.attrs.is_empty() && matches!(&arm.pat, syn::Pat::Ident(ident) if bare_none(ident))
}

fn placeholder() -> syn::Expr {
    syn::Expr::Verbatim(proc_macro2::TokenStream::new())
}

fn wildcard() -> syn::Pat {
    syn::Pat::Wild(syn::PatWild {
        attrs: Vec::new(),
        underscore_token: syn::token::Underscore::default(),
    })
}

fn block_expr(stmts: Vec<syn::Stmt>) -> syn::Expr {
    syn::Expr::Block(syn::ExprBlock {
        attrs: Vec::new(),
        label: None,
        block: syn::Block {
            brace_token: syn::token::Brace::default(),
            stmts,
        },
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
