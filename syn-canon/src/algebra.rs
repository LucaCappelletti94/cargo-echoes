//! Primitive value laws over resolved, bounded expression trees.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString as _};
use alloc::vec::Vec;
use syn::spanned::Spanned as _;
use syn::visit::{self, Visit};
use syn::visit_mut::{self, VisitMut};

use crate::alpha::{Env, Renamer};
use crate::schedule::{self, BitwiseOp, BlockFacts, Equality, Lbl, Port, TotalBinary, Value};
use crate::scope::PrimTy;

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
                    ));
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
        match schedule::total_binary(binary.op, &left, &right) {
            Some(TotalBinary::Flat(op, prim)) => {
                let (expr, label) = self.flat(binary, op, prim, left, right);
                (expr, Some(label))
            }
            Some(TotalBinary::Ordered { tag, reverse, prim }) => {
                let (left, right) = if let Some(reverse) = reverse {
                    core::mem::swap(&mut binary.left, &mut binary.right);
                    binary.op = reverse.bin_op();
                    (right, left)
                } else {
                    (left, right)
                };
                (
                    syn::Expr::Binary(binary),
                    Some(Lbl::Op(tag, Box::new(left), Box::new(right), prim)),
                )
            }
            None => (syn::Expr::Binary(binary), None),
        }
    }

    fn scalar(&self, expr: syn::Expr, total_calls: bool) -> (syn::Expr, Option<Lbl>) {
        if !crate::drift::expr_attrs(&expr).is_empty() {
            return (expr, None);
        }
        match expr {
            syn::Expr::Binary(binary) => self.binary(binary, total_calls),
            syn::Expr::Unary(mut unary) if self.dedup && matches!(unary.op, syn::UnOp::Not(_)) => {
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
            syn::Expr::If(branch) if self.dedup => self.branch(branch, total_calls),
            syn::Expr::Match(r#match) if self.dedup && self.boolean_match(&r#match) => {
                self.branch(conditional(r#match), total_calls)
            }
            syn::Expr::Group(group) => self.scalar(*group.expr, total_calls),
            syn::Expr::MethodCall(mut method) if self.native_wrapping_add(&method) => {
                let label = self.wrapping_add(&mut method);
                (syn::Expr::MethodCall(method), label)
            }
            expr @ syn::Expr::Call(_) if total_calls => {
                let mut locals = 0;
                let label =
                    schedule::label_expr(self.renamer, &self.env, &expr, 0, &mut locals, None).0;
                let label = label.prim().is_some().then_some(label);
                (expr, label)
            }
            expr @ (syn::Expr::Lit(_) | syn::Expr::Path(_) | syn::Expr::Unary(_)) => {
                let label = schedule::scalar_leaf(&self.env, &expr);
                (expr, label)
            }
            expr => (expr, None),
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
        if self.dedup && op == BitwiseOp::Xor {
            pieces.retain(|piece| piece.0.literal_bits() != Some(0));
            if pieces.is_empty() {
                return (
                    crate::constants::literal_expr(prim, 0, span),
                    schedule::literal_label(prim, 0),
                );
            }
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

    fn branch(&self, mut branch: syn::ExprIf, total_calls: bool) -> (syn::Expr, Option<Lbl>) {
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

    /// Whether `method` is `wrapping_add` on a proven fixed-width integer receiver,
    /// which inherent resolution binds to the native by-value method.
    fn native_wrapping_add(&self, method: &syn::ExprMethodCall) -> bool {
        if !schedule::wrapping_add_shape(method) {
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

    /// The label of a native `wrapping_add`, its operands put in label order,
    /// a literal whose type the receiver infers staying the argument.
    fn wrapping_add(&self, method: &mut syn::ExprMethodCall) -> Option<Lbl> {
        let receiver = self.scalar_in_place(&mut method.receiver, true);
        let argument = self.scalar_in_place(&mut method.args[0], true);
        let (receiver, argument) = (receiver?, argument?);
        let prim = schedule::wrapping_add_prim(&receiver, &argument)?;
        if argument < receiver && argument.prim().is_some() {
            core::mem::swap(&mut *method.receiver, &mut method.args[0]);
        }
        Some(schedule::wrapping_add_label(prim, receiver, argument))
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
            | syn::Expr::Group(_) => {
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
            syn::Expr::MethodCall(method) if self.native_wrapping_add(method) => {
                self.wrapping_add(method);
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

fn negate(expr: syn::Expr, label: Lbl) -> (syn::Expr, Lbl) {
    let span = expr.span();
    match (expr, label) {
        (syn::Expr::Unary(unary), Lbl::Not(inner)) => (*unary.expr, *inner),
        (syn::Expr::Binary(binary), Lbl::Op(tag @ ('∧' | '∨'), left, right, _)) => {
            let left = negate(*binary.left, *left);
            let right = negate(*binary.right, *right);
            logical(schedule::dual(tag), left, right, false)
        }
        (syn::Expr::Binary(mut binary), label @ Lbl::Op('<' | '≤' | '=' | '!', ..)) => {
            let complement = schedule::not_label(label);
            binary.op = match &complement {
                Lbl::Op('<', ..) => syn::BinOp::Lt(syn::token::Lt::default()),
                Lbl::Op('≤', ..) => syn::BinOp::Le(syn::token::Le::default()),
                Lbl::Op('=', ..) => syn::BinOp::Eq(syn::token::EqEq::default()),
                _ => syn::BinOp::Ne(syn::token::Ne::default()),
            };
            if matches!(complement, Lbl::Op('<' | '≤', ..)) {
                core::mem::swap(&mut binary.left, &mut binary.right);
            }
            (syn::Expr::Binary(binary), complement)
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
