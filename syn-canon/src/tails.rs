//! Return normalization under resolved temporary-scope proofs.

use crate::alpha::{Env, Renamer};
use crate::schedule::{self, Port, Value};
use crate::scope::{Ns, PrimTy};

pub(crate) fn observed(attrs: &[syn::Attribute], std_names: bool) -> bool {
    attrs.iter().any(|attr| {
        crate::drift::macro_meta(&attr.meta)
            || crate::drift::macro_derive_meta(&attr.meta)
            || (!std_names && (attr.path().is_ident("derive") || attr.path().is_ident("cfg_attr")))
    })
}

pub(crate) fn blocked(renamer: &Renamer<'_>, block: &syn::Block) -> bool {
    if !crate::drift::block_std_names(block) {
        return true;
    }
    let mut statement_macro = false;
    for stmt in &block.stmts {
        match stmt {
            syn::Stmt::Local(local) if observed(&local.attrs, true) => return true,
            syn::Stmt::Item(item) => {
                if observed(crate::helpers::item_attrs(item), true)
                    || matches!(item, syn::Item::Macro(mac) if mac.ident.is_none())
                    || matches!(item, syn::Item::Verbatim(_))
                    || matches!(item, syn::Item::Use(import) if !crate::helpers::use_tree_known(&import.tree, 0))
                {
                    return true;
                }
            }
            syn::Stmt::Macro(_) => statement_macro = true,
            _ => {}
        }
    }
    statement_macro && schedule::unknown_macro_reaches(renamer, block)
}

pub(crate) fn drop_exit_semicolon(renamer: &Renamer<'_>, block: &mut syn::Block) {
    let Some(last) = block.stmts.last() else {
        return;
    };
    let exit = match last {
        syn::Stmt::Expr(expr, Some(_)) => {
            matches!(expr, syn::Expr::Break(_) | syn::Expr::Continue(_))
                && crate::drift::expr_attrs(expr).is_empty()
        }
        syn::Stmt::Macro(mac) if mac.semi_token.is_some() && mac.attrs.is_empty() => {
            matches!(
                schedule::stmt_kind(renamer, last),
                schedule::StmtKind::Boundary(true)
            )
        }
        _ => false,
    };
    if !exit || !schedule::within_bounds(block, 0) {
        return;
    }
    match block.stmts.last_mut() {
        Some(syn::Stmt::Expr(_, semi)) => *semi = None,
        Some(syn::Stmt::Macro(mac)) => mac.semi_token = None,
        _ => {}
    }
}

pub(crate) fn normalize(renamer: &Renamer<'_>, block: &mut syn::Block) {
    if schedule::within_bounds(block, 0) {
        let mut proof = Proof { renamer, locals: 0 };
        proof.block(block, renamer.frame_env(), true);
    }
}

pub(crate) fn normalize_expr(renamer: &Renamer<'_>, expr: &mut syn::Expr) {
    if schedule::expression_within_bounds(expr) {
        let mut proof = Proof { renamer, locals: 0 };
        proof.expr(expr, &renamer.frame_env(), true);
    }
}

pub(crate) fn unwrap(renamer: &Renamer<'_>, body: &mut syn::Expr) {
    let syn::Expr::Block(block) = body else {
        return;
    };
    if block.label.is_some() || !block.attrs.is_empty() {
        return;
    }
    let [syn::Stmt::Expr(expr, None)] = block.block.stmts.as_mut_slice() else {
        return;
    };
    let proof = Proof { renamer, locals: 0 };
    let deferred = matches!(expr, syn::Expr::Closure(_) | syn::Expr::Async(_))
        && crate::drift::expr_attrs(expr).is_empty();
    if !schedule::expression_within_bounds(expr)
        || !(deferred || proof.value(expr, &renamer.frame_env()))
    {
        return;
    }
    *body = core::mem::replace(expr, syn::Expr::Verbatim(proc_macro2::TokenStream::new()));
}

struct Proof<'a, 'env> {
    renamer: &'a Renamer<'env>,
    locals: usize,
}

impl<'env> Proof<'_, 'env> {
    fn bind(&mut self, env: &mut Env<'env>, name: alloc::string::String, prim: Option<PrimTy>) {
        env.insert(
            name,
            Value {
                port: Port::Local(self.locals),
                prim,
                helper: None,
            },
        );
        self.locals += 1;
    }

    fn scope(&mut self, block: &syn::Block, env: &mut Env<'env>) -> Option<bool> {
        let mut types_known = true;
        if blocked(self.renamer, block) {
            return None;
        }
        for stmt in &block.stmts {
            if let syn::Stmt::Item(item) = stmt {
                types_known = false;
                if let syn::Item::Use(import) = item {
                    crate::helpers::walk_imports(
                        &import.tree,
                        &mut alloc::vec::Vec::new(),
                        0,
                        &mut |_, alias| {
                            self.bind(env, alloc::string::String::from(alias), None);
                        },
                    );
                } else if let Some((namespace, ident)) = crate::alpha::item_binding(item)
                    && (namespace == Ns::Value
                        || matches!(item, syn::Item::Struct(item) if !matches!(item.fields, syn::Fields::Named(_))))
                {
                    self.bind(env, crate::scope::ident_name(ident), None);
                }
            }
        }
        Some(types_known)
    }

    fn local(&mut self, local: &syn::Local, env: &mut Env<'env>, types_known: bool) {
        let (pattern, annotated) = match &local.pat {
            syn::Pat::Type(typed) => (typed.pat.as_ref(), Some(typed.ty.as_ref())),
            other => (other, None),
        };
        let prim = if local.attrs.is_empty()
            && matches!(pattern, syn::Pat::Ident(ident) if ident.by_ref.is_none() && ident.subpat.is_none())
        {
            match annotated {
                Some(ty) if types_known => self.renamer.param_prim(ty),
                Some(_) => None,
                None => local
                    .init
                    .as_ref()
                    .filter(|init| init.diverge.is_none())
                    .and_then(|init| self.label_prim(&init.expr, env)),
            }
        } else {
            None
        };
        for name in schedule::pat_names(&local.pat) {
            self.bind(env, name, prim);
        }
    }

    fn block(&mut self, block: &mut syn::Block, mut env: Env<'env>, types_known: bool) -> bool {
        let Some(local_types_known) = self.scope(block, &mut env) else {
            return false;
        };
        let types_known = local_types_known && types_known;
        let Some((last, prefix)) = block.stmts.split_last_mut() else {
            return true;
        };
        for stmt in prefix.iter() {
            if let syn::Stmt::Local(local) = stmt {
                self.local(local, &mut env, types_known);
            }
        }
        if matches!(last, syn::Stmt::Expr(syn::Expr::Return(ret), _) if ret.expr.is_none() && ret.attrs.iter().all(crate::drift::is_inert_attr))
        {
            if matches!(prefix.last(), Some(syn::Stmt::Expr(expr, None)) if !self.value(expr, &env))
            {
                return false;
            }
            block.stmts.pop();
            return true;
        }
        match last {
            syn::Stmt::Expr(expr, semi) => {
                let returned = matches!(expr, syn::Expr::Return(_));
                if semi.is_some() && !returned {
                    if Self::unit(expr, &env) {
                        *semi = None;
                    }
                    return true;
                }
                let safe = self.expr(expr, &env, types_known);
                if returned && !matches!(expr, syn::Expr::Return(_)) {
                    *semi = None;
                }
                safe
            }
            _ => true,
        }
    }

    fn expr(&mut self, expr: &mut syn::Expr, env: &Env<'env>, types_known: bool) -> bool {
        if let syn::Expr::Return(ret) = expr {
            crate::drift::strip_inert_attrs(&mut ret.attrs);
        }
        if !crate::drift::expr_attrs(expr).is_empty() {
            return false;
        }
        match expr {
            syn::Expr::Return(ret) => {
                let safe = ret
                    .expr
                    .as_mut()
                    .is_none_or(|inner| self.expr(inner, env, types_known));
                if safe {
                    *expr = match ret.expr.take() {
                        Some(inner) => *inner,
                        None => syn::Expr::Tuple(syn::ExprTuple {
                            attrs: alloc::vec::Vec::new(),
                            paren_token: syn::token::Paren::default(),
                            elems: syn::punctuated::Punctuated::new(),
                        }),
                    };
                }
                safe
            }
            syn::Expr::Block(block) if block.label.is_none() => {
                self.block(&mut block.block, env.clone(), types_known)
            }
            syn::Expr::Unsafe(block) => self.block(&mut block.block, env.clone(), types_known),
            syn::Expr::If(branch) if branch.else_branch.is_some() => {
                if !self.value(&branch.cond, env) {
                    return false;
                }
                let first = self.block(&mut branch.then_branch, env.clone(), types_known);
                let second = branch
                    .else_branch
                    .as_mut()
                    .is_some_and(|(_, other)| self.expr(other, env, types_known));
                first && second
            }
            syn::Expr::Match(branch) => {
                if !self.value(&branch.expr, env)
                    || branch
                        .arms
                        .iter()
                        .any(|arm| !arm.attrs.is_empty() || !self.guards(&arm.pat, env))
                {
                    return false;
                }
                let mut safe = true;
                for arm in &mut branch.arms {
                    let mut inner = env.clone();
                    for name in schedule::pat_names(&arm.pat) {
                        self.bind(&mut inner, name, None);
                    }
                    safe &= self.expr(&mut arm.body, &inner, types_known);
                }
                safe
            }
            syn::Expr::Group(group) => self.expr(&mut group.expr, env, types_known),
            _ => self.value(expr, env),
        }
    }

    fn guards(&self, pattern: &syn::Pat, env: &Env<'env>) -> bool {
        let mut guards = GuardProof {
            proof: self,
            env,
            pattern,
            safe: true,
        };
        syn::visit::Visit::visit_pat(&mut guards, pattern);
        guards.safe
    }

    fn label_prim(&self, expr: &syn::Expr, env: &Env<'env>) -> Option<PrimTy> {
        if !crate::algebra::within_bounds(expr) {
            return None;
        }
        let mut locals = 0;
        schedule::label_expr(self.renamer, env, expr, 0, &mut locals, None)
            .0
            .prim()
    }

    fn native(expr: &syn::Expr, env: &Env<'env>) -> bool {
        if !crate::drift::expr_attrs(expr).is_empty() {
            return false;
        }
        match expr {
            syn::Expr::Lit(lit) => matches!(
                lit.lit,
                syn::Lit::Int(_)
                    | syn::Lit::Bool(_)
                    | syn::Lit::Float(_)
                    | syn::Lit::Char(_)
                    | syn::Lit::Byte(_)
            ),
            syn::Expr::Group(group) => Self::native(&group.expr, env),
            syn::Expr::Unary(unary)
                if matches!(unary.op, syn::UnOp::Neg(_) | syn::UnOp::Not(_)) =>
            {
                Self::native(&unary.expr, env)
            }
            syn::Expr::Binary(binary) if !schedule::is_assign_op(binary.op) => {
                Self::native(&binary.left, env) && Self::native(&binary.right, env)
            }
            syn::Expr::Path(_) => {
                schedule::scalar_leaf(env, expr).is_some_and(|label| label.prim().is_some())
            }
            _ => false,
        }
    }

    fn unit(expr: &syn::Expr, env: &Env<'env>) -> bool {
        if !crate::drift::expr_attrs(expr).is_empty() {
            return false;
        }
        match expr {
            syn::Expr::Tuple(tuple) => tuple.elems.is_empty(),
            syn::Expr::Assign(assign) => {
                Self::native(&assign.left, env) && Self::native(&assign.right, env)
            }
            syn::Expr::Binary(binary) if schedule::is_assign_op(binary.op) => {
                Self::native(&binary.left, env) && Self::native(&binary.right, env)
            }
            syn::Expr::Group(group) => Self::unit(&group.expr, env),
            _ => false,
        }
    }

    fn value(&self, expr: &syn::Expr, env: &Env<'env>) -> bool {
        if !crate::drift::expr_attrs(expr).is_empty() {
            return false;
        }
        match expr {
            syn::Expr::Lit(_) => true,
            syn::Expr::Tuple(tuple) => tuple.elems.iter().all(|element| self.value(element, env)),
            syn::Expr::Group(group) => self.value(&group.expr, env),
            _ => {
                Self::native(expr, env)
                    || Self::unit(expr, env)
                    || self.label_prim(expr, env).is_some()
            }
        }
    }
}

struct GuardProof<'a, 'r, 'env> {
    proof: &'a Proof<'r, 'env>,
    env: &'a Env<'env>,
    pattern: &'a syn::Pat,
    safe: bool,
}

impl<'ast> syn::visit::Visit<'ast> for GuardProof<'_, '_, '_> {
    fn visit_attribute(&mut self, _: &'ast syn::Attribute) {
        self.safe = false;
    }

    fn visit_pat_guard(&mut self, guard: &'ast syn::PatGuard) {
        let mut env = self.env.clone();
        for name in schedule::pat_names(self.pattern) {
            env.insert(
                name,
                Value {
                    port: Port::Local(0),
                    prim: None,
                    helper: None,
                },
            );
        }
        self.safe &= self.proof.value(&guard.guard, &env);
        syn::visit::visit_pat(self, &guard.pat);
    }
}
