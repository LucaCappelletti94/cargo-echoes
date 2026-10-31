//! The fold the body phases share: a lexical environment of what each local declaration
//! proves, the names a body's own items and imports claim, and one traversal that keeps the
//! environment in step with Rust's scoping while a phase reads each expression.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use syn::visit_mut::VisitMut;

use crate::alpha::Renamer;
use crate::scope::ident_name;

/// The names an item, import or generic parameter declares, and whether a glob import may
/// declare any name.
#[derive(Default)]
pub(crate) struct Claims {
    pub(crate) names: BTreeSet<String>,
    pub(crate) glob: bool,
}

impl Claims {
    /// The value and type names the signature `sig` and body `block` of a function declare.
    pub(crate) fn of_function(sig: &syn::Signature, block: &syn::Block) -> Self {
        let mut scan = ClaimScan {
            claims: Self::default(),
            types_only: false,
        };
        syn::visit::Visit::visit_signature(&mut scan, sig);
        syn::visit::Visit::visit_block(&mut scan, block);
        scan.claims
    }

    /// The type names an item, import or generic parameter inside `block` declares.
    pub(crate) fn of_types(block: &syn::Block) -> Self {
        let mut scan = ClaimScan {
            claims: Self::default(),
            types_only: true,
        };
        syn::visit::Visit::visit_block(&mut scan, block);
        scan.claims
    }
}

struct ClaimScan {
    claims: Claims,
    /// Whether only type-namespace items claim their name.
    types_only: bool,
}

impl syn::visit::Visit<'_> for ClaimScan {
    fn visit_item(&mut self, item: &syn::Item) {
        let ident = if self.types_only {
            crate::alpha::type_item_ident(item)
        } else {
            crate::alpha::item_binding(item).map(|(_, ident)| ident)
        };
        if let Some(ident) = ident {
            self.claims.names.insert(ident_name(ident));
        }
        syn::visit::visit_item(self, item);
    }

    fn visit_use_name(&mut self, name: &syn::UseName) {
        self.claims.names.insert(ident_name(&name.ident));
    }

    fn visit_use_rename(&mut self, rename: &syn::UseRename) {
        self.claims.names.insert(ident_name(&rename.rename));
    }

    fn visit_use_glob(&mut self, _: &syn::UseGlob) {
        self.claims.glob = true;
    }

    fn visit_type_param(&mut self, param: &syn::TypeParam) {
        self.claims.names.insert(ident_name(&param.ident));
        syn::visit::visit_type_param(self, param);
    }
}

/// The bindings in view, innermost scope last, each with what its declaration proves or
/// `None` when it proves nothing.
pub(crate) struct Env<V> {
    scopes: Vec<BTreeMap<String, Option<V>>>,
}

impl<V: Clone> Env<V> {
    fn new() -> Self {
        Self {
            scopes: alloc::vec![BTreeMap::new()],
        }
    }

    /// Bind the names of `pat` in the innermost scope, a lone identifier to `value` and every
    /// other name, including a `ref` or sub-pattern binding, to `None`.
    pub(crate) fn bind(&mut self, pat: &syn::Pat, value: Option<V>) {
        struct Names<'s, V>(&'s mut BTreeMap<String, Option<V>>, Option<V>);
        impl<V: Clone> syn::visit::Visit<'_> for Names<'_, V> {
            fn visit_pat_ident(&mut self, ident: &syn::PatIdent) {
                let value = self
                    .1
                    .clone()
                    .filter(|_| ident.by_ref.is_none() && ident.subpat.is_none());
                self.0.insert(ident_name(&ident.ident), value);
                syn::visit::visit_pat_ident(self, ident);
            }

            fn visit_expr(&mut self, _: &syn::Expr) {}
        }
        let value = value.filter(|_| matches!(pat, syn::Pat::Ident(_)));
        let scope = self.scopes.last_mut().expect("a scope");
        syn::visit::Visit::visit_pat(&mut Names(scope, value), pat);
    }

    /// What the closest declaration of the unattributed single-identifier path `expr` proves.
    pub(crate) fn bound(&self, expr: &syn::Expr) -> Option<V> {
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
            .cloned()
            .flatten()
    }
}

/// A body phase: what its declarations prove and how it reads the body.
pub(crate) trait Phase: Sized {
    /// What a declaration proves about a binding.
    type Value: Clone;

    /// What the declared type `ty` proves.
    fn declared(fold: &PhaseFold<'_, '_, Self>, ty: &syn::Type) -> Option<Self::Value>;

    /// Visit the statement `local` and bind the names it declares.
    fn local(fold: &mut PhaseFold<'_, '_, Self>, local: &mut syn::Local);

    /// Read `expr` after its children.
    fn read(fold: &mut PhaseFold<'_, '_, Self>, expr: &mut syn::Expr);

    /// Read `expr` before its children.
    fn read_first(_: &mut PhaseFold<'_, '_, Self>, _: &mut syn::Expr) {}

    /// Read the iterated expression of a `for` loop before visiting it.
    fn iterable(_: &mut PhaseFold<'_, '_, Self>, _: &mut syn::Expr) {}

    /// Visit the pattern a `match` arm or `let` condition binds, before its names are bound.
    fn bound_pattern(_: &mut PhaseFold<'_, '_, Self>, _: &mut syn::Pat) {}

    /// Visit `pat` wherever the traversal reaches a pattern.
    fn pattern(fold: &mut PhaseFold<'_, '_, Self>, pat: &mut syn::Pat) {
        syn::visit_mut::visit_pat_mut(fold, pat);
    }
}

/// The traversal of one function body by the phase `P`, nested items aside.
pub(crate) struct PhaseFold<'a, 'env, P: Phase> {
    pub(crate) renamer: &'a Renamer<'env>,
    env: Env<P::Value>,
    pub(crate) phase: P,
}

impl<'a, 'env, P: Phase> PhaseFold<'a, 'env, P> {
    pub(crate) fn new(renamer: &'a Renamer<'env>, phase: P) -> Self {
        Self {
            renamer,
            env: Env::new(),
            phase,
        }
    }

    /// Bind the typed inputs of `sig` to what their declared types prove.
    pub(crate) fn bind_inputs(&mut self, sig: &syn::Signature) {
        for input in &sig.inputs {
            if let syn::FnArg::Typed(typed) = input {
                let declared = self.declared(&typed.ty);
                self.bind(&typed.pat, declared);
            }
        }
    }

    /// What the declared type `ty` proves.
    pub(crate) fn declared(&self, ty: &syn::Type) -> Option<P::Value> {
        P::declared(self, ty)
    }

    /// Bind the names of `pat` in the innermost scope.
    pub(crate) fn bind(&mut self, pat: &syn::Pat, value: Option<P::Value>) {
        self.env.bind(pat, value);
    }

    /// What the closest declaration of the single-identifier path `expr` proves.
    pub(crate) fn bound(&self, expr: &syn::Expr) -> Option<P::Value> {
        self.env.bound(expr)
    }

    /// Run `visit` in a fresh innermost scope.
    pub(crate) fn scoped(&mut self, visit: impl FnOnce(&mut Self)) {
        self.env.scopes.push(BTreeMap::new());
        visit(self);
        self.env.scopes.pop();
    }

    /// Visit the initializer and `else` block of `local`.
    pub(crate) fn visit_init(&mut self, local: &mut syn::Local) {
        if let Some(init) = &mut local.init {
            self.visit_expr_mut(&mut init.expr);
            if let Some((_, diverge)) = &mut init.diverge {
                self.visit_expr_mut(diverge);
            }
        }
    }

    /// Visit a condition, binding the patterns of its `let` operands for the guarded block.
    fn condition(&mut self, cond: &mut syn::Expr) {
        match cond {
            syn::Expr::Let(binding) => {
                self.visit_expr_mut(&mut binding.expr);
                P::bound_pattern(self, &mut binding.pat);
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

impl<P: Phase> VisitMut for PhaseFold<'_, '_, P> {
    fn visit_item_mut(&mut self, _: &mut syn::Item) {}

    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        self.scoped(|fold| {
            for stmt in &mut block.stmts {
                fold.visit_stmt_mut(stmt);
            }
        });
    }

    fn visit_local_mut(&mut self, local: &mut syn::Local) {
        P::local(self, local);
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
            P::bound_pattern(fold, &mut arm.pat);
            fold.bind(&arm.pat, None);
            if let syn::Pat::Guard(guard) = &mut arm.pat {
                fold.visit_expr_mut(&mut guard.guard);
            }
            fold.visit_expr_mut(&mut arm.body);
        });
    }

    fn visit_expr_for_loop_mut(&mut self, expr: &mut syn::ExprForLoop) {
        P::iterable(self, &mut expr.expr);
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
        P::pattern(self, pat);
    }

    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        if !crate::drift::expr_attrs(expr).is_empty() {
            return;
        }
        P::read_first(self, expr);
        syn::visit_mut::visit_expr_mut(self, expr);
        P::read(self, expr);
    }
}
