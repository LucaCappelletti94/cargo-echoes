//! Primitive value laws over resolved, bounded expression trees.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString as _};
use alloc::vec::Vec;
use syn::spanned::Spanned as _;
use syn::visit::{self, Visit};
use syn::visit_mut::{self, VisitMut};

use crate::alpha::{Env, Renamer};
use crate::schedule::{
    self, BitwiseOp, BlockFacts, Equality, Identity, Lbl, Port, TotalBinary, Value,
};
use crate::scope::PrimTy;

/// The single unqualified type name a cast targets.
fn cast_name(ty: &syn::Type) -> Option<&syn::Ident> {
    match ty {
        syn::Type::Path(path) if path.qself.is_none() => path.path.get_ident(),
        _ => None,
    }
}

struct Bound {
    depth: usize,
    nodes: usize,
    valid: bool,
}

impl<'ast> Visit<'ast> for Bound {
    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        if !self.valid {
            return;
        }
        self.nodes += 1;
        self.depth += 1;
        if self.depth > schedule::MAX_BLOCK_DEPTH || self.nodes > schedule::MAX_REGION_NODES {
            self.valid = false;
        } else {
            visit::visit_expr(self, expr);
        }
        self.depth -= 1;
    }

    fn visit_block(&mut self, _: &'ast syn::Block) {}
}

pub(crate) fn within_bounds(expr: &syn::Expr) -> bool {
    let mut bound = Bound {
        depth: 0,
        nodes: 0,
        valid: true,
    };
    bound.visit_expr(expr);
    bound.valid
}

pub(crate) fn within_struct_bounds(expr: &syn::ExprStruct) -> bool {
    let mut bound = Bound {
        depth: 1,
        nodes: 1,
        valid: true,
    };
    bound.visit_expr_struct(expr);
    bound.valid
}

pub(crate) fn normalize(renamer: &Renamer<'_>, block: &mut syn::Block, facts: &BlockFacts) {
    if !facts.can_rewrite() {
        return;
    }
    let env = renamer.frame_env();
    let mut walker = Walker {
        renamer,
        env,
        types: BTreeMap::new(),
        locals: 0,
        dedup: true,
    };
    for stmt in &mut block.stmts {
        let definite = matches!(
            schedule::stmt_kind(renamer, stmt),
            schedule::StmtKind::Boundary(true)
        );
        match stmt {
            syn::Stmt::Local(local) => {
                let plan = facts.plan(&local.pat);
                if local.attrs.is_empty()
                    && plan.is_none_or(|plan| plan.active)
                    && let Some(init) = &mut local.init
                    && init.diverge.is_none()
                {
                    walker.visit_expr_mut(&mut init.expr);
                }
                for name in schedule::pat_names(&local.pat) {
                    match plan.and_then(|plan| plan.prim) {
                        Some(prim) => walker.types.insert(name.clone(), prim),
                        None => walker.types.remove(&name),
                    };
                    let value = if let Some(plan) = plan {
                        Value {
                            port: plan.port.clone(),
                            prim: if plan.active { plan.prim } else { None },
                            helper: None,
                        }
                    } else {
                        walker.locals += 1;
                        Value {
                            port: Port::Local(walker.locals),
                            prim: None,
                            helper: None,
                        }
                    };
                    walker.env.insert(name, value);
                }
            }
            syn::Stmt::Expr(expr, _) => {
                walker.visit_expr_mut(expr);
                if let Some(name) = schedule::assign_target(stmt) {
                    if let Some(prim) = walker.binding_prim(&name) {
                        walker.types.insert(name.clone(), prim);
                    }
                    walker.locals += 1;
                    walker.env.insert(
                        name,
                        Value {
                            port: Port::Local(walker.locals),
                            prim: None,
                            helper: None,
                        },
                    );
                }
            }
            syn::Stmt::Item(_) | syn::Stmt::Macro(_) => {}
        }
        if definite {
            break;
        }
    }
}

struct Walker<'a, 'env> {
    renamer: &'a Renamer<'env>,
    env: Env<'env>,
    /// The proven primitive types of the bindings this block declares or assigns, kept
    /// apart from the value environment because an assignment changes a binding's value and
    /// never its type.
    types: BTreeMap<String, PrimTy>,
    locals: usize,
    dedup: bool,
}

fn empty_expr() -> syn::Expr {
    syn::Expr::Verbatim(proc_macro2::TokenStream::new())
}

fn tail_expr_mut(block: &mut syn::Block) -> Option<&mut syn::Expr> {
    let [syn::Stmt::Expr(expr, None)] = block.stmts.as_mut_slice() else {
        return None;
    };
    Some(expr)
}

/// Whether two optional `else` branches are both absent or spell the same tokens.
fn same_branch(
    left: Option<&(syn::token::Else, Box<syn::Expr>)>,
    right: Option<&(syn::token::Else, Box<syn::Expr>)>,
) -> bool {
    use quote::ToTokens as _;
    match (left, right) {
        (None, None) => true,
        (Some((_, left)), Some((_, right))) => {
            left.to_token_stream().to_string() == right.to_token_stream().to_string()
        }
        _ => false,
    }
}

/// The name of a binding `expr` reads by one unattributed identifier, the only place a compound
/// assignment expands without reading a field, an index or a dereference twice.
fn local_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path)
            if path.attrs.is_empty() && path.qself.is_none() && path.path.segments.len() == 1 =>
        {
            Some(crate::scope::unraw(&path.path.segments[0].ident.to_string()).to_string())
        }
        _ => None,
    }
}

impl Walker<'_, '_> {
    fn scalar_in_place(&self, expr: &mut syn::Expr, total_calls: bool) -> Option<Lbl> {
        let owned = core::mem::replace(expr, empty_expr());
        let (normalized, label) = self.scalar(owned, total_calls);
        *expr = normalized;
        label
    }

    /// The proven primitive type of the binding `name` names here, its value proof or the type
    /// it kept through an assignment.
    fn binding_prim(&self, name: &str) -> Option<PrimTy> {
        self.env
            .get(name)
            .and_then(|value| value.prim)
            .or_else(|| self.types.get(name).copied())
    }

    fn binary(&self, mut binary: syn::ExprBinary, total_calls: bool) -> (syn::Expr, Option<Lbl>) {
        let total_calls = total_calls
            || self.dedup
                && (schedule::is_assign_op(binary.op)
                    || matches!(
                        binary.op,
                        syn::BinOp::Eq(_)
                            | syn::BinOp::Ne(_)
                            | syn::BinOp::BitXor(_)
                            | syn::BinOp::And(_)
                            | syn::BinOp::Or(_)
                            | syn::BinOp::Add(_)
                            | syn::BinOp::Mul(_)
                    ));
        if self.dedup
            && matches!(binary.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_))
            && let Some((left, right)) = self.borrowed_operands(&mut binary, total_calls)
        {
            return self.labeled_binary(binary, left, right);
        }
        if self.dedup && matches!(binary.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_)) {
            self.parity(&mut binary.left, &binary.right, total_calls);
            self.parity(&mut binary.right, &binary.left, total_calls);
        }
        let left = self.scalar_in_place(&mut binary.left, total_calls);
        let right = self.scalar_in_place(&mut binary.right, total_calls);
        let (Some(left), Some(right)) = (left, right) else {
            return (syn::Expr::Binary(binary), None);
        };
        if self.dedup
            && let Some(prim) = local_name(&binary.left).and_then(|name| self.binding_prim(&name))
            && let Some(op) = schedule::compound_operator(binary.op, prim, &right)
        {
            let place = binary.left.clone();
            let value = syn::ExprBinary {
                attrs: Vec::new(),
                left: binary.left,
                op,
                right: binary.right,
            };
            let (value, _) = self.binary(value, total_calls);
            let assign = syn::ExprAssign {
                attrs: Vec::new(),
                left: place,
                eq_token: syn::token::Eq::default(),
                right: Box::new(value),
            };
            return (syn::Expr::Assign(assign), None);
        }
        self.labeled_binary(binary, left, right)
    }

    /// Spell `x % 2` as `x & 1` when `x` is a proven fixed-width integer and `other`, the
    /// operand it compares with, is the literal zero. Either remainder sign meets zero alike.
    fn parity(&self, side: &mut syn::Expr, other: &syn::Expr, total_calls: bool) {
        let syn::Expr::Binary(remainder) = side else {
            return;
        };
        if literal_value(other) != Some(0)
            || !remainder.attrs.is_empty()
            || !matches!(remainder.op, syn::BinOp::Rem(_))
            || literal_value(&remainder.right) != Some(2)
        {
            return;
        }
        if let Some(PrimTy::Int { .. }) = self
            .scalar_in_place(&mut remainder.left, total_calls)
            .and_then(|label| label.prim())
        {
            remainder.op = syn::BinOp::BitAnd(syn::token::And::default());
            *remainder.right = unsuffixed(1, remainder.right.span());
        }
    }

    /// The labels of the referents of `&a == &b` or `&a != &b`, both proven primitives, with
    /// the borrows removed, since the standard `PartialEq` for references compares referents.
    fn borrowed_operands(
        &self,
        binary: &mut syn::ExprBinary,
        total_calls: bool,
    ) -> Option<(Lbl, Lbl)> {
        let (syn::Expr::Reference(left), syn::Expr::Reference(right)) =
            (binary.left.as_mut(), binary.right.as_mut())
        else {
            return None;
        };
        if left.mutability.is_some()
            || right.mutability.is_some()
            || !left.attrs.is_empty()
            || !right.attrs.is_empty()
        {
            return None;
        }
        let left_label = self.scalar_in_place(&mut left.expr, total_calls);
        let right_label = self.scalar_in_place(&mut right.expr, total_calls);
        let (Some(left_label), Some(right_label)) = (left_label, right_label) else {
            return None;
        };
        if left_label.prim().is_none() || right_label.prim().is_none() {
            return None;
        }
        let left = core::mem::replace(&mut *left.expr, empty_expr());
        let right = core::mem::replace(&mut *right.expr, empty_expr());
        *binary.left = left;
        *binary.right = right;
        Some((left_label, right_label))
    }

    /// The reduction of a binary operation whose operands both carry labels.
    fn labeled_binary(
        &self,
        mut binary: syn::ExprBinary,
        left: Lbl,
        right: Lbl,
    ) -> (syn::Expr, Option<Lbl>) {
        if self.dedup
            && let Some(equality) = schedule::equality(binary.op, &left, &right)
        {
            let (expr, label) = match equality {
                Equality::Reflexive => (
                    crate::constants::literal_expr(PrimTy::Bool, 1, binary.span()),
                    Lbl::Bool(true),
                ),
                Equality::Operand {
                    left: true,
                    negated,
                } => reduced_operand(*binary.left, left, negated),
                Equality::Operand {
                    left: false,
                    negated,
                } => reduced_operand(*binary.right, right, negated),
            };
            return (expr, Some(label));
        }
        if let Some(tag) = schedule::logical_tag(binary.op)
            && left.prim() == Some(PrimTy::Bool)
            && right.prim() == Some(PrimTy::Bool)
        {
            let (expr, label) = logical(
                tag,
                (*binary.left, left),
                (*binary.right, right),
                self.dedup,
            );
            return (expr, Some(label));
        }
        if self.dedup
            && let Some(identity) = schedule::arithmetic_identity(binary.op, &left, &right)
        {
            let expr = match identity {
                Identity::Left => *binary.left,
                Identity::Right => *binary.right,
                Identity::Zero(prim) => crate::constants::literal_expr(prim, 0, binary.span()),
            };
            return (expr, Some(schedule::identity_label(identity, left, right)));
        }
        if self.dedup
            && let Some((op, value)) = schedule::power_of_two(binary.op, &left, &right)
        {
            binary.op = op;
            *binary.right = unsuffixed(value, binary.right.span());
            return self.labeled_binary(binary, left, Lbl::Int(value, None));
        }
        match schedule::total_binary(binary.op, &left, &right) {
            Some(TotalBinary::Flat(op, prim)) => {
                let (expr, label) = self.flat(binary, op, prim, left, right);
                (expr, Some(label))
            }
            Some(TotalBinary::Ordered { tag, reverse, prim }) => {
                if matches!(tag, '«' | '»') {
                    return (
                        syn::Expr::Binary(binary),
                        Some(Lbl::Op(tag, Box::new(left), Box::new(right), prim)),
                    );
                }
                let (lo, hi) = if reverse {
                    (right.clone(), left.clone())
                } else {
                    (left.clone(), right.clone())
                };
                let (tag, lo, hi) = schedule::adjacent(tag, lo, hi);
                let expr = comparison_tokens(binary, [&left, &right], tag, [&lo, &hi]);
                (expr, Some(Lbl::Op(tag, Box::new(lo), Box::new(hi), prim)))
            }
            None => {
                if schedule::commutative_operands(binary.op, &left, &right) && right < left {
                    core::mem::swap(&mut binary.left, &mut binary.right);
                }
                (syn::Expr::Binary(binary), None)
            }
        }
    }

    /// The comparison chain a `matches!` call, its `match` expansion or a range `contains` call
    /// tests, when the pass may rewrite proven values.
    fn disjunction(&self, expr: &syn::Expr) -> Option<syn::Expr> {
        if !self.dedup
            || !matches!(
                expr,
                syn::Expr::Macro(_) | syn::Expr::Match(_) | syn::Expr::MethodCall(_)
            )
        {
            return None;
        }
        crate::disjunctions::expansion(self.renamer, &self.env, expr)
    }

    fn scalar(&self, expr: syn::Expr, total_calls: bool) -> (syn::Expr, Option<Lbl>) {
        if !crate::drift::expr_attrs(&expr).is_empty() {
            return (expr, None);
        }
        if let Some(chain) = self.disjunction(&expr) {
            return self.scalar(chain, total_calls);
        }
        match expr {
            syn::Expr::Binary(binary) => self.binary(binary, total_calls),
            syn::Expr::Unary(mut unary) if self.dedup && matches!(unary.op, syn::UnOp::Not(_)) => {
                if schedule::double_not(&unary).is_some()
                    && let syn::Expr::Unary(negated) = unary.expr.as_mut()
                    && let Some(label) = self.scalar_in_place(&mut negated.expr, total_calls)
                    && matches!(label.prim(), Some(PrimTy::Int { .. }))
                {
                    return (
                        core::mem::replace(&mut *negated.expr, empty_expr()),
                        Some(label),
                    );
                }
                if let Some(inner) = self.scalar_in_place(&mut unary.expr, total_calls)
                    && inner.prim() == Some(PrimTy::Bool)
                {
                    if !matches!(
                        inner,
                        Lbl::Not(_)
                            | Lbl::Bool(_)
                            | Lbl::Op('∧' | '∨' | '<' | '≤' | '=' | '!', _, _, _)
                    ) {
                        return (syn::Expr::Unary(unary), Some(schedule::not_label(inner)));
                    }
                    let (expr, label) = negate(*unary.expr, inner);
                    return (expr, Some(label));
                }
                (syn::Expr::Unary(unary), None)
            }
            syn::Expr::Unary(mut unary)
                if self.dedup
                    && matches!(unary.op, syn::UnOp::Deref(_))
                    && schedule::shared_borrow(&unary.expr).is_some() =>
            {
                if let syn::Expr::Reference(reference) = unary.expr.as_mut()
                    && let Some(label) = self.scalar_in_place(&mut reference.expr, total_calls)
                    && label.prim().is_some()
                {
                    return (
                        core::mem::replace(&mut *reference.expr, empty_expr()),
                        Some(label),
                    );
                }
                let expr = syn::Expr::Unary(unary);
                let label = schedule::scalar_leaf(&self.env, &expr);
                (expr, label)
            }
            syn::Expr::If(branch) if self.dedup => self.branch(branch, total_calls),
            syn::Expr::Match(r#match) if self.dedup && self.boolean_match(&r#match) => {
                self.branch(conditional(r#match), total_calls)
            }
            syn::Expr::Group(group) => self.scalar(*group.expr, total_calls),
            syn::Expr::Cast(mut cast) if self.dedup => {
                let operand = self.scalar_in_place(&mut cast.expr, total_calls);
                let source = operand.as_ref().and_then(Lbl::prim);
                let target = cast_name(&cast.ty).and_then(|name| self.renamer.type_name_prim(name));
                match (source, target) {
                    (Some(source), Some(target)) if source == target => (*cast.expr, operand),
                    _ => (syn::Expr::Cast(cast), None),
                }
            }
            syn::Expr::Call(call) if self.dedup => self.call(call, total_calls),
            syn::Expr::MethodCall(method) if self.native_commutative(&method) => {
                self.commutative(method)
            }
            syn::Expr::MethodCall(mut method)
                if self.dedup
                    && (schedule::clone_call(&method) && self.renamer.clone_free()
                        || schedule::unit_power(&method)) =>
            {
                let power = schedule::unit_power(&method);
                match self.scalar_in_place(&mut method.receiver, total_calls) {
                    Some(label)
                        if !power && label.prim().is_some()
                            || matches!(label.prim(), Some(PrimTy::Int { .. })) =>
                    {
                        (*method.receiver, Some(label))
                    }
                    _ => (syn::Expr::MethodCall(method), None),
                }
            }
            expr @ (syn::Expr::Lit(_) | syn::Expr::Path(_) | syn::Expr::Unary(_)) => {
                let label = schedule::scalar_leaf(&self.env, &expr);
                (expr, label)
            }
            expr => (expr, None),
        }
    }

    /// The label of a call the enclosing operation proves total, kept when it names a primitive.
    fn total_call(&self, expr: syn::Expr) -> (syn::Expr, Option<Lbl>) {
        let mut locals = 0;
        let label = schedule::label_expr(self.renamer, &self.env, &expr, 0, &mut locals, None).0;
        let label = label.prim().is_some().then_some(label);
        (expr, label)
    }

    /// A call, read as a cast when it is the standard `T::from` of a proven primitive into a
    /// proven primitive `T` it converts to without loss, and as its operand when the types agree.
    fn call(&self, mut call: syn::ExprCall, total_calls: bool) -> (syn::Expr, Option<Lbl>) {
        if let syn::Expr::Path(path) = call.func.as_ref()
            && path.attrs.is_empty()
            && path.qself.is_none()
            && call.args.len() == 2
            && let Some(tag) = self.renamer.standard_cmp(&path.path)
        {
            let left = self.scalar_in_place(&mut call.args[0], true);
            let right = self.scalar_in_place(&mut call.args[1], true);
            let (Some(left), Some(right)) = (left, right) else {
                return (syn::Expr::Call(call), None);
            };
            let Some(prim) = schedule::commutative_prim(tag, &left, &right) else {
                return (syn::Expr::Call(call), None);
            };
            let mut args: Vec<syn::Expr> = call.args.into_iter().collect();
            let second = args.pop().expect("two arguments");
            let first = args.pop().expect("two arguments");
            let (receiver, argument) = if right < left && right.prim().is_some() {
                (second, first)
            } else {
                (first, second)
            };
            let name = if tag == '⊤' { "max" } else { "min" };
            return (
                method_call(receiver, name, argument),
                Some(schedule::commutative_label(tag, prim, left, right)),
            );
        }
        let Some((name, target)) = schedule::from_conversion(self.renamer, &call) else {
            return if total_calls {
                self.total_call(syn::Expr::Call(call))
            } else {
                (syn::Expr::Call(call), None)
            };
        };
        let name = name.clone();
        let operand = self.scalar_in_place(&mut call.args[0], total_calls);
        match operand.as_ref().and_then(Lbl::prim) {
            Some(source) if source == target => {
                (core::mem::replace(&mut call.args[0], empty_expr()), operand)
            }
            Some(source) if schedule::lossless(source, target) => {
                let cast = syn::ExprCast {
                    attrs: Vec::new(),
                    expr: Box::new(core::mem::replace(&mut call.args[0], empty_expr())),
                    as_token: syn::token::As::default(),
                    ty: Box::new(syn::Type::Path(syn::TypePath {
                        attrs: Vec::new(),
                        qself: None,
                        path: syn::Path::from(name),
                    })),
                };
                (syn::Expr::Cast(cast), None)
            }
            _ => (syn::Expr::Call(call), None),
        }
    }

    fn flat(
        &self,
        binary: syn::ExprBinary,
        op: BitwiseOp,
        prim: PrimTy,
        left: Lbl,
        right: Lbl,
    ) -> (syn::Expr, Lbl) {
        let span = binary.span();
        let mut pieces = Vec::new();
        extend(op, prim, left, *binary.left, &mut pieces);
        extend(op, prim, right, *binary.right, &mut pieces);
        pieces.sort_by(|a, b| a.0.cmp(&b.0));
        if self.dedup {
            if op == BitwiseOp::Xor {
                schedule::retain_odd(&mut pieces, |left, right| left.0 == right.0);
            } else {
                pieces.dedup_by(|a, b| a.0 == b.0);
            }
        }
        fold_literals(op, prim, &mut pieces, span);
        if self.dedup
            && let Some(value) = schedule::reduce_literal_leaves(op, prim, &mut pieces, |piece| {
                piece.0.literal_bits()
            })
        {
            return (
                crate::constants::literal_expr(prim, value, span),
                schedule::literal_label(prim, value),
            );
        }
        if pieces.len() == 1 {
            let (label, expr) = pieces.pop().expect("a single operand");
            return match label {
                Lbl::Int(bits, None) => (
                    crate::constants::literal_expr(prim, bits, span),
                    schedule::literal_label(prim, bits),
                ),
                label => (expr, label),
            };
        }
        let count = pieces.len();
        let mut labels = Vec::with_capacity(count);
        let expression = {
            let mut expressions = pieces.into_iter().map(|(label, expr)| {
                labels.push(label);
                expr
            });
            emit(op, &mut expressions, count, span)
        };
        (expression, Lbl::Flat(op.tag(), labels, prim))
    }

    fn condition(&self, expr: syn::Expr, total_calls: bool) -> (syn::Expr, Option<Lbl>, bool) {
        match expr {
            syn::Expr::Unary(mut unary)
                if unary.attrs.is_empty() && matches!(unary.op, syn::UnOp::Not(_)) =>
            {
                let owned = core::mem::replace(&mut *unary.expr, empty_expr());
                let (expr, label, inverted) = self.condition(owned, total_calls);
                if label
                    .as_ref()
                    .is_some_and(|label| label.prim() == Some(PrimTy::Bool))
                {
                    (expr, label, !inverted)
                } else {
                    *unary.expr = expr;
                    (syn::Expr::Unary(unary), None, false)
                }
            }
            syn::Expr::Group(group) if group.attrs.is_empty() => {
                self.condition(*group.expr, total_calls)
            }
            expr => match self.scalar(expr, total_calls) {
                (syn::Expr::Unary(unary), Some(Lbl::Not(inner)))
                    if unary.attrs.is_empty() && matches!(unary.op, syn::UnOp::Not(_)) =>
                {
                    (*unary.expr, Some(*inner), true)
                }
                (expr, label) => (expr, label, false),
            },
        }
    }

    fn tail(&self, block: &mut syn::Block, total_calls: bool) -> Option<Lbl> {
        let expr = tail_expr_mut(block)?;
        self.scalar_in_place(expr, total_calls)
    }

    /// Merge `if p { if q { a } else { b } } else { b }` into `if p && q { a } else { b }`, and
    /// `if p { if q { a } }` into `if p && q { a }`, while the outer `then` block holds only the
    /// inner `if`, both `else` branches are the same tokens and both conditions are proven
    /// total booleans, which `&&` evaluates in the same situations. The outer `if` arrives
    /// unattributed, since `scalar` keeps attributed expressions as written.
    fn collapse(&self, mut branch: syn::ExprIf, total_calls: bool) -> syn::ExprIf {
        loop {
            let last = branch.then_branch.stmts.pop();
            let inner = match (last, branch.then_branch.stmts.is_empty()) {
                (Some(syn::Stmt::Expr(syn::Expr::If(inner), None)), true) => inner,
                (Some(syn::Stmt::Expr(syn::Expr::Match(inner), None)), true)
                    if self.boolean_match(&inner) =>
                {
                    conditional(inner)
                }
                (last, _) => {
                    branch.then_branch.stmts.extend(last);
                    return branch;
                }
            };
            let conditions = if inner.attrs.is_empty()
                && same_branch(branch.else_branch.as_ref(), inner.else_branch.as_ref())
            {
                self.proven_conditions(&branch.cond, &inner.cond, total_calls)
            } else {
                None
            };
            let Some(((outer, outer_label), (nested, nested_label))) = conditions else {
                branch
                    .then_branch
                    .stmts
                    .push(syn::Stmt::Expr(syn::Expr::If(inner), None));
                return branch;
            };
            let (cond, _) = logical('∧', (outer, outer_label), (nested, nested_label), true);
            *branch.cond = cond;
            branch.then_branch = inner.then_branch;
        }
    }

    /// The normalized outer and inner conditions with their labels, when both are proven total
    /// booleans.
    fn proven_conditions(
        &self,
        outer: &syn::Expr,
        inner: &syn::Expr,
        total_calls: bool,
    ) -> Option<((syn::Expr, Lbl), (syn::Expr, Lbl))> {
        let (outer, outer_label) = self.scalar(outer.clone(), total_calls);
        let (inner, inner_label) = self.scalar(inner.clone(), total_calls);
        let (outer_label, inner_label) = (outer_label?, inner_label?);
        (outer_label.prim() == Some(PrimTy::Bool) && inner_label.prim() == Some(PrimTy::Bool))
            .then_some(((outer, outer_label), (inner, inner_label)))
    }

    fn branch(&self, branch: syn::ExprIf, total_calls: bool) -> (syn::Expr, Option<Lbl>) {
        let mut branch = self.collapse(branch, total_calls);
        let Some((_, otherwise)) = &mut branch.else_branch else {
            self.scalar_in_place(&mut branch.cond, total_calls);
            return (syn::Expr::If(branch), None);
        };
        let syn::Expr::Block(else_block) = &mut **otherwise else {
            self.scalar_in_place(&mut branch.cond, total_calls);
            self.scalar_in_place(otherwise, total_calls);
            return (syn::Expr::If(branch), None);
        };
        if !schedule::branches_unattributed(&branch.attrs, &branch.then_branch, else_block) {
            self.scalar_in_place(&mut branch.cond, total_calls);
            return (syn::Expr::If(branch), None);
        }
        let mut then = self.tail(&mut branch.then_branch, total_calls);
        let mut otherwise = self.tail(&mut else_block.block, total_calls);
        let owned = core::mem::replace(&mut *branch.cond, empty_expr());
        let (cond, label, inverted) = self.condition(owned, total_calls);
        *branch.cond = cond;
        let Some(mut cond_label) = label.filter(|label| label.prim() == Some(PrimTy::Bool)) else {
            return (syn::Expr::If(branch), None);
        };
        if inverted {
            core::mem::swap(&mut branch.then_branch, &mut else_block.block);
            core::mem::swap(&mut then, &mut otherwise);
        }
        let complementary = matches!(
            (&then, &otherwise),
            (Some(Lbl::Bool(true)), Some(Lbl::Bool(false)))
                | (Some(Lbl::Bool(false)), Some(Lbl::Bool(true)))
        );
        if !complementary && schedule::positive_condition(&cond_label).is_some() {
            let owned = core::mem::replace(&mut *branch.cond, empty_expr());
            let (cond, positive) = negate(owned, cond_label);
            *branch.cond = cond;
            cond_label = positive;
            core::mem::swap(&mut branch.then_branch, &mut else_block.block);
            core::mem::swap(&mut then, &mut otherwise);
        }
        let (Some(then), Some(otherwise)) = (then, otherwise) else {
            return (syn::Expr::If(branch), None);
        };
        if let Some(value) = schedule::integer_choice(self.renamer, &cond_label, &then, &otherwise)
        {
            let then_expr = core::mem::replace(
                tail_expr_mut(&mut branch.then_branch).expect("proven scalar tail"),
                empty_expr(),
            );
            let else_expr = core::mem::replace(
                tail_expr_mut(&mut else_block.block).expect("proven scalar tail"),
                empty_expr(),
            );
            let cond = (*branch.cond, cond_label);
            let expr = choice_expr(&value, cond, (then_expr, &then), (else_expr, &otherwise));
            return (expr, Some(value));
        }
        let Some(value) = schedule::branch_value(&cond_label, &then, &otherwise) else {
            return (syn::Expr::If(branch), None);
        };
        if matches!(value, Lbl::Select(..)) {
            return (syn::Expr::If(branch), Some(value));
        }
        if let Lbl::Bool(selected) = cond_label {
            let block = if selected {
                &mut branch.then_branch
            } else {
                &mut else_block.block
            };
            let expr = tail_expr_mut(block).expect("proven scalar tail");
            return (core::mem::replace(expr, empty_expr()), Some(value));
        }
        if !complementary
            && let Some((tag, negated, keep_then)) = schedule::literal_branch(&then, &otherwise)
        {
            let (block, operand) = if keep_then {
                (&mut branch.then_branch, then)
            } else {
                (&mut else_block.block, otherwise)
            };
            let expr = tail_expr_mut(block).expect("proven scalar tail");
            let operand = (core::mem::replace(expr, empty_expr()), operand);
            let cond = if negated {
                negate(*branch.cond, cond_label)
            } else {
                (*branch.cond, cond_label)
            };
            let (expr, label) = logical(tag, cond, operand, true);
            return (expr, Some(label));
        }
        if then == Lbl::Bool(true) {
            (*branch.cond, Some(value))
        } else {
            let (expr, label) = negate(*branch.cond, cond_label);
            (expr, Some(label))
        }
    }

    /// Whether `method` is a commutative inherent method on a proven fixed-width integer
    /// receiver, which inherent resolution binds to the native by-value method.
    fn native_commutative(&self, method: &syn::ExprMethodCall) -> bool {
        if schedule::commutative_method(self.renamer, method).is_none() {
            return false;
        }
        let mut locals = 0;
        let (receiver, _) = schedule::label_expr(
            self.renamer,
            &self.env,
            &method.receiver,
            0,
            &mut locals,
            None,
        );
        matches!(receiver.prim(), Some(PrimTy::Int { .. }))
    }

    /// The value of a native commutative method call: its operands put in label order, a
    /// literal whose type the receiver infers staying the argument, or the other operand when
    /// one operand is the method's identity.
    fn commutative(&self, mut method: syn::ExprMethodCall) -> (syn::Expr, Option<Lbl>) {
        let tag = schedule::commutative_method(self.renamer, &method)
            .expect("a native commutative method");
        let receiver = self.scalar_in_place(&mut method.receiver, true);
        let argument = self.scalar_in_place(&mut method.args[0], true);
        let (Some(receiver), Some(argument)) = (receiver, argument) else {
            return (syn::Expr::MethodCall(method), None);
        };
        let Some(prim) = schedule::commutative_prim(tag, &receiver, &argument) else {
            return (syn::Expr::MethodCall(method), None);
        };
        let unit = schedule::method_identity(tag);
        if self.dedup && unit.is_some() && argument.literal_bits() == unit {
            return (*method.receiver, Some(receiver));
        }
        if self.dedup && unit.is_some() && receiver.literal_bits() == unit {
            let argument_expr = method.args.pop().expect("one argument");
            return (argument_expr, Some(argument));
        }
        if argument < receiver && argument.prim().is_some() {
            core::mem::swap(&mut *method.receiver, &mut method.args[0]);
        }
        let label = schedule::commutative_label(tag, prim, receiver, argument);
        (syn::Expr::MethodCall(method), Some(label))
    }

    /// Whether `r#match` is a boolean match its conditional form proves.
    fn boolean_match(&self, r#match: &syn::ExprMatch) -> bool {
        let mut locals = 0;
        schedule::boolean_match_value(self.renamer, &self.env, r#match, 0, &mut locals, None)
            .is_some()
    }
}

/// The conditional form of an admitted boolean match, its scrutinee the
/// condition and its `true` arm the first branch.
fn conditional(r#match: syn::ExprMatch) -> syn::ExprIf {
    let mut arms = r#match.arms.into_iter();
    let first = arms.next().expect("an admitted first arm");
    let second = arms.next().expect("an admitted second arm");
    let first_true = matches!(
        &first.pat,
        syn::Pat::Lit(syn::ExprLit { lit: syn::Lit::Bool(literal), .. }) if literal.value
    );
    let (then, otherwise) = if first_true {
        (first, second)
    } else {
        (second, first)
    };
    syn::ExprIf {
        attrs: Vec::new(),
        if_token: syn::token::If::default(),
        cond: r#match.expr,
        then_branch: branch_block(*then.body),
        else_branch: Some((
            syn::token::Else::default(),
            Box::new(syn::Expr::Block(syn::ExprBlock {
                attrs: Vec::new(),
                label: None,
                block: branch_block(*otherwise.body),
            })),
        )),
    }
}

/// An arm body as a branch block, a bare expression becoming its tail.
fn branch_block(body: syn::Expr) -> syn::Block {
    match body {
        syn::Expr::Block(block) if block.label.is_none() => block.block,
        body => syn::Block {
            brace_token: syn::token::Brace::default(),
            stmts: alloc::vec![syn::Stmt::Expr(body, None)],
        },
    }
}

impl VisitMut for Walker<'_, '_> {
    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        if !crate::drift::expr_attrs(expr).is_empty() {
            return;
        }
        if let Some(chain) = self.disjunction(expr) {
            *expr = self.scalar(chain, false).0;
            return;
        }
        match expr {
            syn::Expr::Match(r#match) if self.dedup && self.boolean_match(r#match) => {
                let owned = core::mem::replace(expr, empty_expr());
                *expr = self.scalar(owned, false).0;
            }
            syn::Expr::Binary(_)
            | syn::Expr::Unary(_)
            | syn::Expr::If(_)
            | syn::Expr::Lit(_)
            | syn::Expr::Path(_)
            | syn::Expr::Group(_)
            | syn::Expr::Cast(_) => {
                let owned = core::mem::replace(expr, empty_expr());
                *expr = self.scalar(owned, false).0;
            }
            syn::Expr::Call(call)
                if self.dedup
                    && (schedule::from_conversion(self.renamer, call).is_some()
                        || matches!(call.func.as_ref(), syn::Expr::Path(path) if self.renamer.standard_cmp(&path.path).is_some())) =>
            {
                let owned = core::mem::replace(expr, empty_expr());
                *expr = self.scalar(owned, false).0;
            }
            syn::Expr::Reference(reference) => {
                let dedup = core::mem::replace(&mut self.dedup, false);
                self.visit_expr_mut(&mut reference.expr);
                self.dedup = dedup;
            }
            syn::Expr::RawAddr(raw) => {
                let dedup = core::mem::replace(&mut self.dedup, false);
                self.visit_expr_mut(&mut raw.expr);
                self.dedup = dedup;
            }
            syn::Expr::MethodCall(method)
                if self.native_commutative(method)
                    || self.dedup
                        && (schedule::clone_call(method) && self.renamer.clone_free()
                            || schedule::unit_power(method)) =>
            {
                let owned = core::mem::replace(expr, empty_expr());
                *expr = self.scalar(owned, false).0;
            }
            syn::Expr::MethodCall(method) => {
                let dedup = core::mem::replace(&mut self.dedup, false);
                self.visit_expr_mut(&mut method.receiver);
                self.dedup = dedup;
                for argument in &mut method.args {
                    self.visit_expr_mut(argument);
                }
            }
            syn::Expr::Block(_)
            | syn::Expr::Const(_)
            | syn::Expr::Unsafe(_)
            | syn::Expr::Async(_)
            | syn::Expr::TryBlock(_)
            | syn::Expr::Closure(_)
            | syn::Expr::Match(_)
            | syn::Expr::While(_)
            | syn::Expr::ForLoop(_)
            | syn::Expr::Loop(_)
            | syn::Expr::Let(_)
            | syn::Expr::Macro(_)
            | syn::Expr::Verbatim(_) => {}
            _ => visit_mut::visit_expr_mut(self, expr),
        }
    }
}

/// The operand a literal boolean comparison reduces to, negated when the comparison negates it.
fn reduced_operand(expr: syn::Expr, label: Lbl, negated: bool) -> (syn::Expr, Lbl) {
    if negated {
        negate(expr, label)
    } else {
        (expr, label)
    }
}

/// The canonical chain of `left` and `right` joined by the logical operator `tag` over total
/// booleans, nested chains of the same operator flattened into one. With `reduce`, repeated
/// operands and identity literals leave and an absorbing literal decides the chain.
fn logical(
    tag: char,
    left: (syn::Expr, Lbl),
    right: (syn::Expr, Lbl),
    reduce: bool,
) -> (syn::Expr, Lbl) {
    fn collect(tag: char, (expr, label): (syn::Expr, Lbl), out: &mut Vec<(Lbl, syn::Expr)>) {
        match (expr, label) {
            (syn::Expr::Binary(binary), Lbl::Op(inner, left, right, _))
                if inner == tag
                    && binary.attrs.is_empty()
                    && schedule::logical_tag(binary.op) == Some(tag) =>
            {
                collect(tag, (*binary.left, *left), out);
                collect(tag, (*binary.right, *right), out);
            }
            (expr, label) => out.push((label, expr)),
        }
    }
    let span = left.0.span();
    let mut operands = Vec::new();
    collect(tag, left, &mut operands);
    collect(tag, right, &mut operands);
    if let Some(value) = schedule::canonical_operands(tag, &mut operands, reduce) {
        return (
            crate::constants::literal_expr(PrimTy::Bool, u128::from(value), span),
            Lbl::Bool(value),
        );
    }
    let op = if tag == '∧' {
        syn::BinOp::And(syn::token::AndAnd::default())
    } else {
        syn::BinOp::Or(syn::token::OrOr::default())
    };
    let (label, expr) =
        schedule::balanced(operands, &mut |(left_label, left), (right_label, right)| {
            (
                Lbl::Op(
                    tag,
                    Box::new(left_label),
                    Box::new(right_label),
                    PrimTy::Bool,
                ),
                syn::Expr::Binary(syn::ExprBinary {
                    attrs: Vec::new(),
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                }),
            )
        });
    (expr, label)
}
/// The spelling of an integer choice `value`: the `Ord` method over the two branch values in
/// label order, or the cast of the condition, negated when `then` holds the `0`.
fn choice_expr(
    value: &Lbl,
    cond: (syn::Expr, Lbl),
    then: (syn::Expr, &Lbl),
    otherwise: (syn::Expr, &Lbl),
) -> syn::Expr {
    if let Lbl::Flat(tag, ..) = value {
        let (receiver, argument) = if otherwise.1 < then.1 && otherwise.1.prim().is_some() {
            (otherwise.0, then.0)
        } else {
            (then.0, otherwise.0)
        };
        return method_call(receiver, if *tag == '⊤' { "max" } else { "min" }, argument);
    }
    let (operand, _) = if then.1.literal_bits() == Some(1) {
        cond
    } else {
        negate(cond.0, cond.1)
    };
    let name = then
        .1
        .prim()
        .and_then(schedule::int_type_ident)
        .expect("an integer choice of a fixed-width type");
    syn::Expr::Cast(syn::ExprCast {
        attrs: Vec::new(),
        expr: Box::new(operand),
        as_token: syn::token::As::default(),
        ty: Box::new(syn::Type::Path(syn::TypePath {
            attrs: Vec::new(),
            qself: None,
            path: syn::Path::from(name),
        })),
    })
}

/// The method call `receiver.name(argument)`.
fn method_call(receiver: syn::Expr, name: &str, argument: syn::Expr) -> syn::Expr {
    syn::Expr::MethodCall(syn::ExprMethodCall {
        attrs: Vec::new(),
        receiver: Box::new(receiver),
        dot_token: syn::token::Dot::default(),
        method: syn::Ident::new(name, proc_macro2::Span::call_site()),
        turbofish: None,
        paren_token: syn::token::Paren::default(),
        args: core::iter::once(argument).collect(),
    })
}

/// The tokens of `lo tag hi` over the operands of `binary`, labeled `operands`. An integer
/// literal takes the spelling of its label, and every other operand moves to the place its label
/// holds in `target`.
fn comparison_tokens(
    mut binary: syn::ExprBinary,
    operands: [&Lbl; 2],
    tag: char,
    target: [&Lbl; 2],
) -> syn::Expr {
    let span = binary.span();
    let mut exprs = [
        Some(core::mem::replace(&mut *binary.left, empty_expr())),
        Some(core::mem::replace(&mut *binary.right, empty_expr())),
    ];
    let mut place = |label: &Lbl| match label {
        Lbl::Int(bits, Some(prim)) => crate::constants::literal_expr(*prim, *bits, span),
        Lbl::Int(bits, None) => syn::Expr::Lit(syn::ExprLit {
            attrs: Vec::new(),
            lit: syn::Lit::Int(syn::LitInt::new(&bits.to_string(), span)),
        }),
        _ => {
            let index = (0..2)
                .find(|&index| exprs[index].is_some() && operands[index] == label)
                .expect("a comparison operand keeps its label");
            exprs[index].take().expect("an operand placed once")
        }
    };
    *binary.left = place(target[0]);
    *binary.right = place(target[1]);
    binary.op = match tag {
        '<' => syn::BinOp::Lt(syn::token::Lt::default()),
        '≤' => syn::BinOp::Le(syn::token::Le::default()),
        '=' => syn::BinOp::Eq(syn::token::EqEq::default()),
        _ => syn::BinOp::Ne(syn::token::Ne::default()),
    };
    syn::Expr::Binary(binary)
}

fn negate(expr: syn::Expr, label: Lbl) -> (syn::Expr, Lbl) {
    let span = expr.span();
    match (expr, label) {
        (syn::Expr::Unary(unary), Lbl::Not(inner)) => (*unary.expr, *inner),
        (syn::Expr::Binary(binary), Lbl::Op(tag @ ('∧' | '∨'), left, right, _)) => {
            let left = negate(*binary.left, *left);
            let right = negate(*binary.right, *right);
            logical(schedule::dual(tag), left, right, false)
        }
        (syn::Expr::Binary(binary), Lbl::Op(tag @ ('<' | '≤' | '=' | '!'), left, right, prim)) => {
            let (tag, lo, hi) = schedule::complement(tag, (*left).clone(), (*right).clone());
            let expr = comparison_tokens(binary, [&left, &right], tag, [&lo, &hi]);
            (expr, Lbl::Op(tag, Box::new(lo), Box::new(hi), prim))
        }
        (_, Lbl::Bool(value)) => (
            syn::Expr::Lit(syn::ExprLit {
                attrs: Vec::new(),
                lit: syn::Lit::Bool(syn::LitBool::new(!value, span)),
            }),
            Lbl::Bool(!value),
        ),
        (expr, label) => (
            syn::Expr::Unary(syn::ExprUnary {
                attrs: Vec::new(),
                op: syn::UnOp::Not(syn::token::Not { spans: [span] }),
                expr: Box::new(expr),
            }),
            schedule::not_label(label),
        ),
    }
}

fn extend(
    op: BitwiseOp,
    prim: PrimTy,
    label: Lbl,
    expr: syn::Expr,
    out: &mut Vec<(Lbl, syn::Expr)>,
) {
    match label {
        Lbl::Flat(inner_op, labels, inner_prim) if inner_op == op.tag() && inner_prim == prim => {
            let mut expressions = Vec::with_capacity(labels.len());
            decompose(op, expr, &mut expressions);
            assert_eq!(
                labels.len(),
                expressions.len(),
                "one expression per operand"
            );
            out.extend(labels.into_iter().zip(expressions));
        }
        label => out.push((label, expr)),
    }
}

fn decompose(op: BitwiseOp, expr: syn::Expr, out: &mut Vec<syn::Expr>) {
    match expr {
        syn::Expr::Binary(binary) if BitwiseOp::from_bin_op(binary.op) == Some(op) => {
            decompose(op, *binary.left, out);
            decompose(op, *binary.right, out);
        }
        expr => out.push(expr),
    }
}

fn fold_literals(
    op: BitwiseOp,
    prim: PrimTy,
    pieces: &mut Vec<(Lbl, syn::Expr)>,
    span: proc_macro2::Span,
) {
    let literal = |label: &Lbl| label.literal_bits().is_some();
    if pieces.iter().filter(|piece| literal(&piece.0)).count() < 2 {
        return;
    }
    let mut acc = None;
    pieces.retain(|piece| {
        let Some(bits) = piece.0.literal_bits() else {
            return true;
        };
        acc = Some(match acc {
            None => bits,
            Some(left) => op.fold(left, bits),
        });
        false
    });
    let bits = acc.expect("at least two literal operands");
    let label = schedule::literal_label(prim, bits);
    pieces.push((label, crate::constants::literal_expr(prim, bits, span)));
    pieces.sort_by(|a, b| a.0.cmp(&b.0));
}

fn emit(
    op: BitwiseOp,
    expressions: &mut impl Iterator<Item = syn::Expr>,
    count: usize,
    span: proc_macro2::Span,
) -> syn::Expr {
    if count == 1 {
        return expressions
            .next()
            .expect("an operand for each counted leaf");
    }
    let left = emit(op, expressions, count / 2, span);
    let right = emit(op, expressions, count - count / 2, span);
    let op = match op {
        BitwiseOp::And => syn::BinOp::BitAnd(syn::token::And { spans: [span] }),
        BitwiseOp::Or => syn::BinOp::BitOr(syn::token::Or { spans: [span] }),
        BitwiseOp::Xor => syn::BinOp::BitXor(syn::token::Caret { spans: [span] }),
    };
    syn::Expr::Binary(syn::ExprBinary {
        attrs: Vec::new(),
        left: Box::new(left),
        op,
        right: Box::new(right),
    })
}

/// The unsuffixed integer literal `value`.
fn unsuffixed(value: u128, span: proc_macro2::Span) -> syn::Expr {
    syn::Expr::Lit(syn::ExprLit {
        attrs: Vec::new(),
        lit: syn::Lit::Int(syn::LitInt::new(&value.to_string(), span)),
    })
}

/// The value of an unattributed integer literal.
fn literal_value(expr: &syn::Expr) -> Option<u128> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            attrs,
            lit: syn::Lit::Int(literal),
        }) if attrs.is_empty() => literal.base10_parse().ok(),
        _ => None,
    }
}
