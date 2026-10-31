//! Small syntax builders and queries the readings share.

use alloc::boxed::Box;
use alloc::vec::Vec;

/// The empty expression that holds a place while its node moves.
pub(crate) fn placeholder() -> syn::Expr {
    syn::Expr::Verbatim(proc_macro2::TokenStream::new())
}

/// The identifier `name` at the call site.
pub(crate) fn ident(name: &str) -> syn::Ident {
    syn::Ident::new(name, proc_macro2::Span::call_site())
}

/// The wildcard pattern `_`.
pub(crate) fn wild() -> syn::Pat {
    syn::Pat::Wild(syn::PatWild {
        attrs: Vec::new(),
        underscore_token: syn::token::Underscore::default(),
    })
}

/// The `match` arm `pat => body` without a trailing comma.
pub(crate) fn arm(pat: syn::Pat, body: syn::Expr) -> syn::Arm {
    syn::Arm {
        attrs: Vec::new(),
        pat,
        fat_arrow_token: syn::token::FatArrow::default(),
        body: Box::new(body),
        comma: None,
    }
}

/// The `bool` literal `value`.
pub(crate) fn bool_expr(value: bool) -> syn::Expr {
    syn::Expr::Lit(syn::ExprLit {
        attrs: Vec::new(),
        lit: syn::Lit::Bool(syn::LitBool::new(value, proc_macro2::Span::call_site())),
    })
}

/// Whether the syntax `walk` visits leaves any type to inference with `_`.
pub(crate) fn infers<'ast>(walk: impl FnOnce(&mut dyn syn::visit::Visit<'ast>)) -> bool {
    struct Infer(bool);
    impl syn::visit::Visit<'_> for Infer {
        fn visit_type_infer(&mut self, _: &syn::TypeInfer) {
            self.0 = true;
        }
    }
    let mut infer = Infer(false);
    walk(&mut infer);
    infer.0
}
