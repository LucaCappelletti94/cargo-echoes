//! Literal disjunctions and literal ranges over a proven integer or `char` place.
//!
//! A standard `matches!` without a guard whose patterns are integer or `char` literals and
//! literal ranges, the `match` it expands to, the inherent `contains` of a literal range, and the
//! inherent ASCII predicates of `char` and `u8` read as the chain of comparisons they test:
//! `matches!(x, 1 | 3..=5)` as `x == 1 || x >= 3 && x <= 5` and `c.is_ascii_digit()` as
//! `c >= '0' && c <= '9'`. The scrutinee must be a binding of a proven fixed-width integer or
//! `char` type, which the chain may read once per comparison, and the alternatives must be
//! disjoint, so every spelling of one set of values meets one chain whose order the boolean
//! readings settle.

use alloc::boxed::Box;
use alloc::string::ToString as _;
use alloc::vec::Vec;

use crate::alpha::{Env, Renamer};
use crate::scope::{IntWidth, PrimTy};

/// The comparison chain the `matches!` call, its `match` expansion, the range `contains` call or
/// the ASCII predicate `expr` tests.
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
        syn::Expr::Match(expr_match) => {
            let pattern = matches_arm(expr_match)?;
            let mut alternatives = Vec::new();
            pattern_alternatives(pattern, &mut alternatives)?;
            ((*expr_match.expr).clone(), alternatives)
        }
        syn::Expr::MethodCall(call) if call.method == "contains" => contained(renamer, call)?,
        syn::Expr::MethodCall(call) => {
            let ranges = ascii_ranges(renamer, call)?;
            let place = (*call.receiver).clone();
            let alternatives = match place_prim(env, &place)? {
                PrimTy::Char => ranges
                    .iter()
                    .map(|&(start, end)| ascii_alternative(start, end, char_literal))
                    .collect(),
                PrimTy::Int {
                    width: IntWidth::W8,
                    signed: false,
                } => ranges
                    .iter()
                    .map(|&(start, end)| ascii_alternative(start, end, byte_literal))
                    .collect(),
                _ => return None,
            };
            (place, alternatives)
        }
        _ => return None,
    };
    let prim = place_prim(env, &place)?;
    disjoint(prim, &alternatives)?;
    alternatives
        .into_iter()
        .map(|alternative| alternative.comparison(&place))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .reduce(|left, right| binary(left, syn::BinOp::Or(syn::token::OrOr::default()), right))
}

/// The proven primitive type of the single-identifier binding `place`.
fn place_prim(env: &Env<'_>, place: &syn::Expr) -> Option<PrimTy> {
    let syn::Expr::Path(path) = place else {
        return None;
    };
    let ident = path
        .path
        .get_ident()
        .filter(|_| path.attrs.is_empty() && path.qself.is_none())?;
    let name = crate::scope::unraw(&ident.to_string()).to_string();
    env.get(&name)?.prim
}

/// The inclusive code ranges the ASCII predicate called by the unattributed, argument-free
/// `call` holds for, when the standard predicate answers it.
fn ascii_ranges(renamer: &Renamer<'_>, call: &syn::ExprMethodCall) -> Option<&'static [(u8, u8)]> {
    if !call.attrs.is_empty()
        || call.turbofish.is_some()
        || !call.args.is_empty()
        || !renamer.ascii_free()
    {
        return None;
    }
    let ranges: &'static [(u8, u8)] = match call.method.to_string().as_str() {
        "is_ascii_digit" => &[(b'0', b'9')],
        "is_ascii_lowercase" => &[(b'a', b'z')],
        "is_ascii_uppercase" => &[(b'A', b'Z')],
        "is_ascii_alphabetic" => &[(b'A', b'Z'), (b'a', b'z')],
        "is_ascii_alphanumeric" => &[(b'0', b'9'), (b'A', b'Z'), (b'a', b'z')],
        "is_ascii_hexdigit" => &[(b'0', b'9'), (b'A', b'F'), (b'a', b'f')],
        "is_ascii_whitespace" => &[
            (b'\t', b'\t'),
            (b'\n', b'\n'),
            (b'\x0C', b'\x0C'),
            (b'\r', b'\r'),
            (b' ', b' '),
        ],
        _ => return None,
    };
    Some(ranges)
}

/// The alternative of the inclusive code range from `start` to `end`, a literal when both meet,
/// with literals spelled by `literal`.
fn ascii_alternative(start: u8, end: u8, literal: fn(u8) -> syn::Lit) -> Alternative {
    if start == end {
        return Alternative::Literal(literal(start));
    }
    Alternative::Range {
        start: Some(literal(start)),
        end: Some(literal(end)),
        closed: true,
    }
}

/// The `char` literal of the ASCII code `code`.
fn char_literal(code: u8) -> syn::Lit {
    syn::Lit::Char(syn::LitChar::new(
        char::from(code),
        proc_macro2::Span::call_site(),
    ))
}

/// The unsuffixed integer literal of the byte `code`.
fn byte_literal(code: u8) -> syn::Lit {
    syn::Lit::Int(syn::LitInt::new(
        &code.to_string(),
        proc_macro2::Span::call_site(),
    ))
}

/// The pattern of `match e { P => true, _ => false }`, the expansion of `matches!(e, P)`.
fn matches_arm(expr_match: &syn::ExprMatch) -> Option<&syn::Pat> {
    let [matched, rest] = expr_match.arms.as_slice() else {
        return None;
    };
    let literal = |arm: &syn::Arm, expected: bool| {
        arm.attrs.is_empty()
            && matches!(arm.body.as_ref(), syn::Expr::Lit(syn::ExprLit { attrs, lit: syn::Lit::Bool(value) }) if attrs.is_empty() && value.value == expected)
    };
    (expr_match.attrs.is_empty()
        && literal(matched, true)
        && literal(rest, false)
        && matches!(&rest.pat, syn::Pat::Wild(wild) if wild.attrs.is_empty()))
    .then_some(&matched.pat)
}

/// One alternative: an integer or `char` literal, or a range with optional literal bounds.
enum Alternative {
    Literal(syn::Lit),
    Range {
        start: Option<syn::Lit>,
        end: Option<syn::Lit>,
        closed: bool,
    },
}

impl Alternative {
    /// The comparisons of `place` that test the alternative, none for a range without bounds.
    fn comparison(self, place: &syn::Expr) -> Option<syn::Expr> {
        let compare = |op, literal: syn::Lit| binary(place.clone(), op, literal_expr(literal));
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

/// Collect the alternatives of `pat`, or fail on a pattern other than an integer or `char`
/// literal or a range of them.
fn pattern_alternatives(pat: &syn::Pat, out: &mut Vec<Alternative>) -> Option<()> {
    match pat {
        syn::Pat::Or(or) if or.attrs.is_empty() => {
            for case in &or.cases {
                pattern_alternatives(case, out)?;
            }
        }
        syn::Pat::Paren(paren) if paren.attrs.is_empty() => pattern_alternatives(&paren.pat, out)?,
        syn::Pat::Lit(literal) => out.push(Alternative::Literal(scalar(literal)?.clone())),
        syn::Pat::Range(range) => out.push(range_alternative(range)?),
        _ => return None,
    }
    Some(())
}

/// The integer or `char` literal an unattributed literal expression spells.
fn scalar(expr: &syn::ExprLit) -> Option<&syn::Lit> {
    (expr.attrs.is_empty() && matches!(expr.lit, syn::Lit::Int(_) | syn::Lit::Char(_)))
        .then_some(&expr.lit)
}

/// The alternative of an unattributed range whose bounds are integer or `char` literals.
fn range_alternative(range: &syn::ExprRange) -> Option<Alternative> {
    range.attrs.is_empty().then_some(())?;
    let bound = |bound: &Option<Box<syn::Expr>>| -> Option<Option<syn::Lit>> {
        match bound.as_deref() {
            None => Some(None),
            Some(syn::Expr::Lit(literal)) => scalar(literal).cloned().map(Some),
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

/// Whether every literal belongs to `prim`, an integer type or `char` ordered by scalar value,
/// and the alternatives cover disjoint, nonempty sets.
fn disjoint(prim: PrimTy, alternatives: &[Alternative]) -> Option<()> {
    let (min, max) = match prim {
        PrimTy::Int { width, signed } => {
            let bits = width.bits();
            if signed {
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
            }
        }
        PrimTy::Char => (0, i128::from(u32::from(char::MAX))),
        PrimTy::Bool => return None,
    };
    let value = |literal: &syn::Lit| -> Option<i128> {
        let value = match (prim, literal) {
            (PrimTy::Int { .. }, syn::Lit::Int(literal))
                if literal.suffix().is_empty() || literal.suffix() == prim.suffix() =>
            {
                literal.base10_parse::<i128>().ok()?
            }
            (PrimTy::Char, syn::Lit::Char(literal)) if literal.suffix().is_empty() => {
                i128::from(u32::from(literal.value()))
            }
            _ => return None,
        };
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

/// The expression of a literal, a negative integer one as the negation of its magnitude.
fn literal_expr(literal: syn::Lit) -> syn::Expr {
    if let syn::Lit::Int(integer) = &literal
        && let Some(magnitude) = integer.to_string().strip_prefix('-')
    {
        return syn::Expr::Unary(syn::ExprUnary {
            attrs: Vec::new(),
            op: syn::UnOp::Neg(syn::token::Minus::default()),
            expr: Box::new(lit(syn::Lit::Int(syn::LitInt::new(
                magnitude,
                integer.span(),
            )))),
        });
    }
    lit(literal)
}

fn lit(literal: syn::Lit) -> syn::Expr {
    syn::Expr::Lit(syn::ExprLit {
        attrs: Vec::new(),
        lit: literal,
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
