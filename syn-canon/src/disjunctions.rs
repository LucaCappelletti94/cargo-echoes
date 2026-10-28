//! Literal disjunctions and literal ranges over a proven integer place.
//!
//! A standard `matches!` without a guard whose patterns are integer literals and literal ranges,
//! and the inherent `contains` of a literal range, read as the chain of comparisons they test:
//! `matches!(x, 1 | 3..=5)` as `x == 1 || x >= 3 && x <= 5`. The scrutinee must be a binding of a
//! proven fixed-width integer type, which the chain may read once per comparison, and the
//! alternatives must be disjoint, so every spelling of one set of values meets one chain whose
//! order the boolean readings settle.

use alloc::boxed::Box;
use alloc::string::ToString as _;
use alloc::vec::Vec;

use crate::alpha::{Env, Renamer};
use crate::scope::{IntWidth, PrimTy};

/// The comparison chain the `matches!` call or the range `contains` call `expr` tests.
pub(crate) fn expansion(
    renamer: &Renamer<'_>,
    env: &Env<'_>,
    expr: &syn::Expr,
) -> Option<syn::Expr> {
    let (place, alternatives) = match expr {
        syn::Expr::Macro(mac)
            if mac.attrs.is_empty() && renamer.standard_matches(&mac.mac.path) =>
        {
            let args = syn::parse2::<crate::macros::MatchesArgs>(mac.mac.tokens.clone()).ok()?;
            if args.guard.is_some() {
                return None;
            }
            let mut alternatives = Vec::new();
            pattern_alternatives(&args.pattern, &mut alternatives)?;
            (args.value, alternatives)
        }
        syn::Expr::MethodCall(call) => contained(renamer, call)?,
        _ => return None,
    };
    let syn::Expr::Path(path) = &place else {
        return None;
    };
    let ident = path
        .path
        .get_ident()
        .filter(|_| path.attrs.is_empty() && path.qself.is_none())?;
    let name = crate::scope::unraw(&ident.to_string()).to_string();
    let PrimTy::Int { width, signed } = env.get(&name)?.prim? else {
        return None;
    };
    disjoint(width, signed, &alternatives)?;
    alternatives
        .into_iter()
        .map(|alternative| alternative.comparison(&place))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .reduce(|left, right| binary(left, syn::BinOp::Or(syn::token::OrOr::default()), right))
}

/// One alternative: a literal, or a range with optional literal bounds.
enum Alternative {
    Literal(syn::LitInt),
    Range {
        start: Option<syn::LitInt>,
        end: Option<syn::LitInt>,
        closed: bool,
    },
}

impl Alternative {
    /// The comparisons of `place` that test the alternative, none for a range without bounds.
    fn comparison(self, place: &syn::Expr) -> Option<syn::Expr> {
        let compare = |op, literal: syn::LitInt| binary(place.clone(), op, literal_expr(&literal));
        match self {
            Self::Literal(literal) => Some(compare(
                syn::BinOp::Eq(syn::token::EqEq::default()),
                literal,
            )),
            Self::Range { start, end, closed } => {
                let start =
                    start.map(|start| compare(syn::BinOp::Ge(syn::token::Ge::default()), start));
                let end = end.map(|end| {
                    let op = if closed {
                        syn::BinOp::Le(syn::token::Le::default())
                    } else {
                        syn::BinOp::Lt(syn::token::Lt::default())
                    };
                    compare(op, end)
                });
                start.into_iter().chain(end).reduce(|start, end| {
                    binary(start, syn::BinOp::And(syn::token::AndAnd::default()), end)
                })
            }
        }
    }
}

/// Collect the alternatives of `pat`, or fail on a pattern other than an integer literal or a
/// range of integer literals.
fn pattern_alternatives(pat: &syn::Pat, out: &mut Vec<Alternative>) -> Option<()> {
    match pat {
        syn::Pat::Or(or) if or.attrs.is_empty() => {
            for case in &or.cases {
                pattern_alternatives(case, out)?;
            }
        }
        syn::Pat::Paren(paren) if paren.attrs.is_empty() => pattern_alternatives(&paren.pat, out)?,
        syn::Pat::Lit(literal) => out.push(Alternative::Literal(integer(literal)?.clone())),
        syn::Pat::Range(range) => out.push(range_alternative(range)?),
        _ => return None,
    }
    Some(())
}

/// The integer literal an unattributed literal expression spells.
fn integer(expr: &syn::ExprLit) -> Option<&syn::LitInt> {
    match expr {
        syn::ExprLit {
            attrs,
            lit: syn::Lit::Int(literal),
        } if attrs.is_empty() => Some(literal),
        _ => None,
    }
}

/// The alternative of an unattributed range whose bounds are integer literals.
fn range_alternative(range: &syn::ExprRange) -> Option<Alternative> {
    range.attrs.is_empty().then_some(())?;
    let bound = |bound: &Option<Box<syn::Expr>>| -> Option<Option<syn::LitInt>> {
        match bound.as_deref() {
            None => Some(None),
            Some(syn::Expr::Lit(literal)) => integer(literal).cloned().map(Some),
            Some(_) => None,
        }
    };
    Some(Alternative::Range {
        start: bound(&range.start)?,
        end: bound(&range.end)?,
        closed: matches!(range.limits, syn::RangeLimits::Closed(_)),
    })
}

/// The place and range of an unattributed `(a..=b).contains(&x)` without turbofish, whose
/// `contains` the collection proof keeps the inherent method of the standard range. The drift
/// has already dropped the parentheses around the range.
fn contained(
    renamer: &Renamer<'_>,
    call: &syn::ExprMethodCall,
) -> Option<(syn::Expr, Vec<Alternative>)> {
    if !call.attrs.is_empty()
        || call.turbofish.is_some()
        || call.method != "contains"
        || call.args.len() != 1
        || !renamer.collection_free()
    {
        return None;
    }
    let syn::Expr::Range(range) = &*call.receiver else {
        return None;
    };
    let syn::Expr::Reference(reference) = &call.args[0] else {
        return None;
    };
    if !reference.attrs.is_empty() || reference.mutability.is_some() {
        return None;
    }
    Some((
        (*reference.expr).clone(),
        alloc::vec![range_alternative(range)?],
    ))
}

/// Whether every literal belongs to the integer type of `width` and `signed` and the
/// alternatives cover disjoint, nonempty sets.
fn disjoint(width: IntWidth, signed: bool, alternatives: &[Alternative]) -> Option<()> {
    let suffix = PrimTy::Int { width, signed }.suffix();
    let bits = width.bits();
    let (min, max) = if signed {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else {
        (
            0,
            if bits == 128 {
                i128::MAX
            } else {
                (1i128 << bits) - 1
            },
        )
    };
    let value = |literal: &syn::LitInt| -> Option<i128> {
        if !literal.suffix().is_empty() && literal.suffix() != suffix {
            return None;
        }
        let value = literal.base10_parse::<i128>().ok()?;
        (min..=max).contains(&value).then_some(value)
    };
    let mut spans = Vec::with_capacity(alternatives.len());
    for alternative in alternatives {
        spans.push(match alternative {
            Alternative::Literal(literal) => {
                let value = value(literal)?;
                (value, value)
            }
            Alternative::Range { start, end, closed } => {
                let low = start.as_ref().map_or(Some(min), value)?;
                let high = match end {
                    None => max,
                    Some(end) if *closed => value(end)?,
                    Some(end) => value(end)?.checked_sub(1)?,
                };
                if low > high {
                    return None;
                }
                (low, high)
            }
        });
    }
    spans.sort_unstable();
    spans
        .windows(2)
        .all(|pair| pair[0].1 < pair[1].0)
        .then_some(())
}

/// The expression of an integer literal, a negative one as the negation of its magnitude.
fn literal_expr(literal: &syn::LitInt) -> syn::Expr {
    let text = literal.to_string();
    match text.strip_prefix('-') {
        Some(magnitude) => syn::Expr::Unary(syn::ExprUnary {
            attrs: Vec::new(),
            op: syn::UnOp::Neg(syn::token::Minus::default()),
            expr: Box::new(lit(syn::LitInt::new(magnitude, literal.span()))),
        }),
        None => lit(literal.clone()),
    }
}

fn lit(literal: syn::LitInt) -> syn::Expr {
    syn::Expr::Lit(syn::ExprLit {
        attrs: Vec::new(),
        lit: syn::Lit::Int(literal),
    })
}

fn binary(left: syn::Expr, op: syn::BinOp, right: syn::Expr) -> syn::Expr {
    syn::Expr::Binary(syn::ExprBinary {
        attrs: Vec::new(),
        left: Box::new(left),
        op,
        right: Box::new(right),
    })
}
