//! A function view spells each file-level import of an external crate item as the item's
//! rooted path before any reading inspects the body, so the standard path recognizers see
//! one spelling.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use proc_macro2::{Ident, Span};
use syn::visit::Visit;
use syn::visit_mut::VisitMut;

use crate::scope::{Frame, Ns, ident_name};

/// The aliases of `seed` that name one external crate item in every namespace they occupy,
/// each with the item's rooted path.
pub(crate) fn aliases(seed: &Frame) -> BTreeMap<String, Vec<String>> {
    let mut paths: BTreeMap<&str, Option<&[String]>> = BTreeMap::new();
    for ns in [Ns::Value, Ns::Type, Ns::Macro] {
        for (name, binding) in seed.bindings(ns) {
            let external = seed.external(binding);
            paths
                .entry(name.as_str())
                .and_modify(|path| {
                    if *path != external {
                        *path = None;
                    }
                })
                .or_insert(external);
        }
    }
    paths
        .into_iter()
        .filter_map(|(name, path)| Some((String::from(name), path?.to_vec())))
        .collect()
}

/// Every name the function binds anywhere, as a pattern, generic parameter, nested item or
/// nested import, and whether a glob import or a live attribute inside it blocks the spelling.
#[derive(Default)]
struct Bound {
    names: BTreeSet<String>,
    blocked: bool,
}

impl Visit<'_> for Bound {
    fn visit_attribute(&mut self, attr: &syn::Attribute) {
        self.blocked |= crate::context::live(&attr.meta);
    }

    fn visit_pat_ident(&mut self, pat: &syn::PatIdent) {
        self.names.insert(ident_name(&pat.ident));
        syn::visit::visit_pat_ident(self, pat);
    }

    fn visit_type_param(&mut self, param: &syn::TypeParam) {
        self.names.insert(ident_name(&param.ident));
        syn::visit::visit_type_param(self, param);
    }

    fn visit_const_param(&mut self, param: &syn::ConstParam) {
        self.names.insert(ident_name(&param.ident));
        syn::visit::visit_const_param(self, param);
    }

    fn visit_item(&mut self, item: &syn::Item) {
        let ident = match item {
            syn::Item::Const(item) => Some(&item.ident),
            syn::Item::Enum(item) => Some(&item.ident),
            syn::Item::ExternCrate(item) => {
                Some(item.rename.as_ref().map_or(&item.ident, |(_, alias)| alias))
            }
            syn::Item::Fn(item) => Some(&item.sig.ident),
            syn::Item::Macro(item) => item.ident.as_ref(),
            syn::Item::Mod(item) => Some(&item.ident),
            syn::Item::Static(item) => Some(&item.ident),
            syn::Item::Struct(item) => Some(&item.ident),
            syn::Item::Trait(item) => Some(&item.ident),
            syn::Item::TraitAlias(item) => Some(&item.ident),
            syn::Item::Type(item) => Some(&item.ident),
            syn::Item::Union(item) => Some(&item.ident),
            _ => None,
        };
        if let Some(ident) = ident {
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
        self.blocked = true;
    }
}

struct Root<'a> {
    aliases: BTreeMap<&'a str, &'a [String]>,
    /// The next path follows a bare `<T>`, so its head names an associated item of `T`.
    keep_head: bool,
}

impl Root<'_> {
    /// Visit `node`, whose path may follow a qualified self. The trait path of
    /// `<T as Trait>::item` roots like any path, with the qualified self still counting the
    /// trait path's segments, and `<T>::item` keeps its head.
    fn qualified<N>(
        &mut self,
        node: &mut N,
        parts: fn(&mut N) -> (&mut Option<syn::QSelf>, &mut syn::Path),
        visit: fn(&mut Self, &mut N),
    ) {
        let (slot, path) = parts(node);
        let Some(mut qself) = slot.take() else {
            visit(self, node);
            return;
        };
        let before = path.segments.len();
        self.keep_head = qself.position == 0;
        visit(self, node);
        let (slot, path) = parts(node);
        if qself.position > 0 {
            qself.position += path.segments.len() - before;
        }
        self.visit_qself_mut(&mut qself);
        *slot = Some(qself);
    }
}

impl VisitMut for Root<'_> {
    fn visit_path_mut(&mut self, path: &mut syn::Path) {
        if !core::mem::take(&mut self.keep_head)
            && path.leading_colon.is_none()
            && let Some(first) = path.segments.first()
            && let Some(external) = self.aliases.get(ident_name(&first.ident).as_str())
        {
            let arguments = core::mem::replace(
                &mut path
                    .segments
                    .first_mut()
                    .expect("a first segment")
                    .arguments,
                syn::PathArguments::None,
            );
            let mut rooted: syn::punctuated::Punctuated<syn::PathSegment, syn::Token![::]> =
                external
                    .iter()
                    .map(|name| syn::PathSegment::from(Ident::new(name, Span::call_site())))
                    .collect();
            rooted.last_mut().expect("a rooted external path").arguments = arguments;
            rooted.extend(core::mem::take(&mut path.segments).into_iter().skip(1));
            path.segments = rooted;
        }
        syn::visit_mut::visit_path_mut(self, path);
    }

    fn visit_expr_path_mut(&mut self, expr: &mut syn::ExprPath) {
        self.qualified(
            expr,
            |expr| (&mut expr.qself, &mut expr.path),
            syn::visit_mut::visit_expr_path_mut,
        );
    }

    fn visit_type_path_mut(&mut self, ty: &mut syn::TypePath) {
        self.qualified(
            ty,
            |ty| (&mut ty.qself, &mut ty.path),
            syn::visit_mut::visit_type_path_mut,
        );
    }

    fn visit_expr_struct_mut(&mut self, expr: &mut syn::ExprStruct) {
        self.qualified(
            expr,
            |expr| (&mut expr.qself, &mut expr.path),
            syn::visit_mut::visit_expr_struct_mut,
        );
    }

    fn visit_pat_struct_mut(&mut self, pat: &mut syn::PatStruct) {
        self.qualified(
            pat,
            |pat| (&mut pat.qself, &mut pat.path),
            syn::visit_mut::visit_pat_struct_mut,
        );
    }

    fn visit_pat_tuple_struct_mut(&mut self, pat: &mut syn::PatTupleStruct) {
        self.qualified(
            pat,
            |pat| (&mut pat.qself, &mut pat.path),
            syn::visit_mut::visit_pat_tuple_struct_mut,
        );
    }

    // Attribute and macro paths resolve through their own namespaces and readings.
    fn visit_attribute_mut(&mut self, _: &mut syn::Attribute) {}

    fn visit_macro_mut(&mut self, _: &mut syn::Macro) {}
}

/// Spell every path of `function` whose first segment one of the module's `aliases` binds to
/// an external crate item as that item's rooted path. A name the function binds anywhere, or
/// one an enclosing generic parameter takes, keeps its spelling, and so does every path once
/// the function holds a glob import or binds the first segment of a rooted path.
pub(crate) fn root(
    function: &mut syn::ItemFn,
    aliases: &BTreeMap<String, Vec<String>>,
    generics: Option<&Frame>,
) {
    if aliases.is_empty() {
        return;
    }
    let mut bound = Bound::default();
    bound.visit_item_fn(function);
    if bound.blocked {
        return;
    }
    if let Some(generics) = generics {
        for ns in [Ns::Value, Ns::Type] {
            bound.names.extend(generics.bindings(ns).keys().cloned());
        }
    }
    let aliases: BTreeMap<&str, &[String]> = aliases
        .iter()
        .filter(|(alias, path)| {
            !bound.names.contains(*alias)
                && path.first().is_some_and(|root| !bound.names.contains(root))
        })
        .map(|(alias, path)| (alias.as_str(), path.as_slice()))
        .collect();
    if aliases.is_empty() {
        return;
    }
    let mut visitor = Root {
        aliases,
        keep_head: false,
    };
    visitor.visit_signature_mut(&mut function.sig);
    visitor.visit_block_mut(&mut function.block);
}
