//! Conversions, defaults and annotations whose type a declaration fixes.
//!
//! A `let` annotation or the declared return type of a function tail fixes the target of
//! `x.into()` and `Default::default()`. With a primitive or `String` target and a source whose
//! declared type has a standard `From` impl into it, `x.into()` calls that impl through the
//! blanket `Into`, which no user impl between standard types can replace, so it reads as
//! `T::from(x)`. `Default::default()` and `T::default()` of a primitive, `String` or `Vec` read
//! as the zero literal, `false`, `'\0'` or the empty constructor the standard impl returns, and
//! `mem::replace(x, d)` with such a default reads as `mem::take(x)`, which the standard library
//! defines as that call.
//!
//! A `let` annotation that repeats the type its initializer already fixes reads as that
//! initializer alone: `let y: P = x` with `x` of the primitive `P` as `let y = x`, and
//! `let v: C<T…> = C::new()` or `C::with_capacity(n)` of a standard collection `C` as
//! `let v = C::<T…>::new()` and its `with_capacity` counterpart.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use syn::visit_mut::VisitMut;

use crate::alpha::Renamer;
use crate::scope::{PrimTy, ident_name};

/// Read the declared-target conversions and defaults of the body `block` of a function with the
/// signature `sig`, nested items aside.
pub(crate) fn normalize(renamer: &Renamer<'_>, sig: &syn::Signature, block: &mut syn::Block) {
    let mut claims = Claims::default();
    syn::visit::Visit::visit_signature(&mut claims, sig);
    syn::visit::Visit::visit_block(&mut claims, block);
    let standard = STANDARD_NAMES
        .into_iter()
        .filter(|name| !claims.glob && !claims.names.contains(*name) && renamer.prelude_trait(name))
        .collect();
    let mut fold = Fold {
        renamer,
        standard,
        scopes: alloc::vec![BTreeMap::new()],
    };
    for input in &sig.inputs {
        if let syn::FnArg::Typed(typed) = input {
            let declared = fold.declared(&typed.ty);
            fold.bind(&typed.pat, declared);
        }
    }
    let target = match &sig.output {
        syn::ReturnType::Type(_, ty) => fold.declared(ty),
        syn::ReturnType::Default => None,
    };
    fold.scoped(|fold| {
        for stmt in &mut block.stmts {
            fold.visit_stmt_mut(stmt);
        }
        if let (Some(target), Some(syn::Stmt::Expr(tail, None))) = (target, block.stmts.last_mut())
        {
            fold.targeted(tail, &target);
        }
    });
}

/// The names an item, import or generic parameter of the function declares, and whether a
/// glob import may declare any name.
#[derive(Default)]
struct Claims {
    names: BTreeSet<String>,
    glob: bool,
}

impl syn::visit::Visit<'_> for Claims {
    fn visit_item(&mut self, item: &syn::Item) {
        if let Some((_, ident)) = crate::alpha::item_binding(item) {
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

/// What a declared type proves about a value.
#[derive(Clone)]
enum Declared {
    /// A primitive, with the single type name that spells it.
    Prim(PrimTy, syn::Ident),
    /// The standard `String`.
    String,
    /// A shared `str` reference.
    Str,
    /// The standard `Vec`.
    Vec,
}

/// The standard names the readings rely on: the prelude traits `Into`, `From` and `Default`,
/// the prelude types `String` and `Vec`, and the primitive `str`.
const STANDARD_NAMES: [&str; 6] = ["Into", "From", "Default", "String", "Vec", "str"];

struct Fold<'a, 'env> {
    renamer: &'a Renamer<'env>,
    /// The names of `STANDARD_NAMES` that resolve to their standard item here.
    standard: BTreeSet<&'static str>,
    /// The bindings in view, with what their declared type proves.
    scopes: Vec<BTreeMap<String, Option<Declared>>>,
}

impl Fold<'_, '_> {
    /// Whether every one of `names` resolves to its standard item.
    fn proves(&self, names: &[&str]) -> bool {
        names.iter().all(|name| self.standard.contains(name))
    }

    /// What the declared type `ty` proves.
    fn declared(&self, ty: &syn::Type) -> Option<Declared> {
        match ty {
            syn::Type::Reference(reference) if reference.mutability.is_none() => {
                let syn::Type::Path(path) = reference.elem.as_ref() else {
                    return None;
                };
                (self.proves(&["str"]) && path.qself.is_none() && path.path.is_ident("str"))
                    .then_some(Declared::Str)
            }
            syn::Type::Path(path) if path.qself.is_none() && path.path.leading_colon.is_none() => {
                let [segment] = path.path.segments.iter().collect::<Vec<_>>()[..] else {
                    return None;
                };
                match (ident_name(&segment.ident).as_str(), &segment.arguments) {
                    ("String", syn::PathArguments::None) if self.proves(&["String"]) => {
                        Some(Declared::String)
                    }
                    ("Vec", syn::PathArguments::AngleBracketed(args))
                        if self.proves(&["Vec"]) && args.args.len() == 1 =>
                    {
                        Some(Declared::Vec)
                    }
                    (_, syn::PathArguments::None) => self
                        .renamer
                        .type_name_prim(&segment.ident)
                        .map(|prim| Declared::Prim(prim, segment.ident.clone())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Bind the names of `pat`, a lone identifier to `declared` and every other name to
    /// `None`.
    fn bind(&mut self, pat: &syn::Pat, declared: Option<Declared>) {
        struct Names<'s>(&'s mut BTreeMap<String, Option<Declared>>, Option<Declared>);
        impl syn::visit::Visit<'_> for Names<'_> {
            fn visit_pat_ident(&mut self, ident: &syn::PatIdent) {
                let declared = self
                    .1
                    .clone()
                    .filter(|_| ident.by_ref.is_none() && ident.subpat.is_none());
                self.0.insert(ident_name(&ident.ident), declared);
                syn::visit::visit_pat_ident(self, ident);
            }

            fn visit_expr(&mut self, _: &syn::Expr) {}
        }
        let declared = declared.filter(|_| matches!(pat, syn::Pat::Ident(_)));
        let scope = self.scopes.last_mut().expect("a scope");
        syn::visit::Visit::visit_pat(&mut Names(scope, declared), pat);
    }

    /// What the type of `expr` proves, a single-identifier binding by its declaration and a
    /// literal by its spelling.
    fn source(&self, expr: &syn::Expr) -> Option<Declared> {
        match expr {
            syn::Expr::Path(path) if path.attrs.is_empty() && path.qself.is_none() => {
                let name = ident_name(path.path.get_ident()?);
                self.scopes
                    .iter()
                    .rev()
                    .find_map(|scope| scope.get(&name))
                    .cloned()
                    .flatten()
            }
            syn::Expr::Lit(literal) if literal.attrs.is_empty() => {
                let prim = match &literal.lit {
                    syn::Lit::Str(text) if text.suffix().is_empty() && self.proves(&["str"]) => {
                        return Some(Declared::Str);
                    }
                    syn::Lit::Char(character) if character.suffix().is_empty() => PrimTy::Char,
                    syn::Lit::Bool(_) => PrimTy::Bool,
                    syn::Lit::Int(integer) => {
                        let prim = crate::scope::primitive_suffix(integer.suffix())?;
                        if !matches!(prim, PrimTy::Int { .. }) {
                            return None;
                        }
                        prim
                    }
                    _ => return None,
                };
                Some(Declared::Prim(
                    prim,
                    syn::Ident::new(prim.suffix(), proc_macro2::Span::call_site()),
                ))
            }
            _ => None,
        }
    }

    /// Read `x.into()` and `Default::default()` whose type the declaration `target` fixes.
    fn targeted(&self, expr: &mut syn::Expr, target: &Declared) {
        match expr {
            syn::Expr::MethodCall(call)
                if self.proves(&["Into", "From"])
                    && call.attrs.is_empty()
                    && call.turbofish.is_none()
                    && call.args.is_empty()
                    && call.method == "into" =>
            {
                let Some(source) = self.source(&call.receiver) else {
                    return;
                };
                let owner = match (target, &source) {
                    (Declared::Prim(to, name), Declared::Prim(from, _))
                        if from == to || crate::schedule::lossless(*from, *to) =>
                    {
                        name.clone()
                    }
                    (
                        Declared::String,
                        Declared::Str | Declared::String | Declared::Prim(PrimTy::Char, _),
                    ) => ident("String"),
                    _ => return,
                };
                let value = core::mem::replace(call.receiver.as_mut(), placeholder());
                *expr = associated_call(owner, None, "from", Some(value));
            }
            syn::Expr::Call(call) if self.proves(&["Default"]) && default_call(call) => {
                *expr = match target {
                    Declared::Prim(prim, _) => {
                        crate::constants::literal_expr(*prim, 0, proc_macro2::Span::call_site())
                    }
                    Declared::String => associated_call(ident("String"), None, "new", None),
                    Declared::Vec => associated_call(ident("Vec"), None, "new", None),
                    Declared::Str => syn::Expr::Lit(syn::ExprLit {
                        attrs: Vec::new(),
                        lit: syn::Lit::Str(syn::LitStr::new("", proc_macro2::Span::call_site())),
                    }),
                };
            }
            _ => {}
        }
    }

    /// Read `T::default()` of a primitive, `String` or `Vec` as the value it returns.
    fn standard_default(&self, expr: &mut syn::Expr) {
        if !self.proves(&["Default"]) {
            return;
        }
        let syn::Expr::Call(call) = expr else {
            return;
        };
        let Some((owner, arguments)) = owner_call(call, "default") else {
            return;
        };
        *expr = match ident_name(&owner).as_str() {
            "String" if self.proves(&["String"]) && arguments.is_none() => {
                associated_call(owner, None, "new", None)
            }
            "Vec" if self.proves(&["Vec"]) => associated_call(owner, arguments, "new", None),
            _ => match self.renamer.type_name_prim(&owner) {
                Some(prim) if arguments.is_none() => {
                    crate::constants::literal_expr(prim, 0, proc_macro2::Span::call_site())
                }
                _ => return,
            },
        };
    }

    /// Whether `expr` is a default the standard `Default` impl of the replaced value's type
    /// returns: `Default::default()`, or `T::default()` or `T::new()` of a primitive, `String`
    /// or `Vec` for `default` and of `String` or `Vec` for `new`.
    fn replacement_default(&self, expr: &syn::Expr) -> bool {
        let syn::Expr::Call(call) = expr else {
            return false;
        };
        if default_call(call) {
            return true;
        }
        let standard = |owner: &syn::Ident, arguments: &Option<_>| match ident_name(owner).as_str()
        {
            "String" => self.proves(&["String"]) && arguments.is_none(),
            "Vec" => self.proves(&["Vec"]),
            _ => false,
        };
        if let Some((owner, arguments)) = owner_call(call, "new") {
            return standard(&owner, &arguments);
        }
        owner_call(call, "default").is_some_and(|(owner, arguments)| {
            standard(&owner, &arguments)
                || arguments.is_none() && self.renamer.type_name_prim(&owner).is_some()
        })
    }

    /// Read `mem::replace(x, d)` through a standard crate root as `mem::take(x)` when `d` is a
    /// standard default.
    fn take(&self, expr: &mut syn::Expr) {
        if !self.proves(&["Default"]) {
            return;
        }
        let syn::Expr::Call(call) = expr else {
            return;
        };
        let replaces = call.attrs.is_empty()
            && call.args.len() == 2
            && self.renamer.standard_function(&call.func, "mem", "replace")
            && self.replacement_default(&call.args[1]);
        if replaces
            && let syn::Expr::Path(path) = call.func.as_mut()
            && let Some(last) = path.path.segments.last_mut()
        {
            last.ident = syn::Ident::new("take", last.ident.span());
            call.args.pop();
            call.args.pop_punct();
        }
    }

    /// Drop the annotation of a `let` binding one identifier whose initializer already fixes
    /// the annotated type, a primitive value or a standard collection constructor, moving a
    /// collection's type arguments onto the constructor.
    fn annotation(&self, local: &mut syn::Local, declared: Option<&Declared>) {
        let syn::Pat::Type(typed) = &local.pat else {
            return;
        };
        let Some(init) = &mut local.init else {
            return;
        };
        let plain = matches!(typed.pat.as_ref(), syn::Pat::Ident(ident)
            if ident.attrs.is_empty() && ident.by_ref.is_none() && ident.subpat.is_none());
        if !local.attrs.is_empty() || !typed.attrs.is_empty() || !plain || init.diverge.is_some() {
            return;
        }
        let stated = match (declared, self.source(&init.expr)) {
            (Some(Declared::Prim(to, _)), Some(Declared::Prim(from, _))) => *to == from,
            _ => self.constructor(&typed.ty, &mut init.expr),
        };
        if stated {
            let binding = typed.pat.as_ref().clone();
            local.pat = binding;
        }
    }

    /// Move the type arguments of the annotation `ty` of a standard collection onto the
    /// initializer `init` when it calls that collection's `new` or `with_capacity`, reporting
    /// whether it did.
    fn constructor(&self, ty: &syn::Type, init: &mut syn::Expr) -> bool {
        let syn::Type::Path(annotation) = ty else {
            return false;
        };
        let Some(collection) = self.renamer.standard_collection(&annotation.path) else {
            return false;
        };
        let arguments = match annotation
            .path
            .segments
            .last()
            .map(|segment| &segment.arguments)
        {
            Some(syn::PathArguments::None) if collection == "String" => None,
            Some(syn::PathArguments::AngleBracketed(arguments))
                if collection != "String" && !infers(arguments) =>
            {
                Some(arguments.clone())
            }
            _ => return false,
        };
        let syn::Expr::Call(call) = init else {
            return false;
        };
        let syn::Expr::Path(callee) = call.func.as_mut() else {
            return false;
        };
        let segments = callee.path.segments.iter().cloned().collect::<Vec<_>>();
        let Some((method, owner)) = segments.split_last().filter(|(_, owner)| !owner.is_empty())
        else {
            return false;
        };
        let owner = syn::Path {
            leading_colon: callee.path.leading_colon,
            segments: owner.iter().cloned().collect(),
        };
        let arity = match ident_name(&method.ident).as_str() {
            "new" => 0,
            "with_capacity" => 1,
            _ => return false,
        };
        if annotation.qself.is_some()
            || !call.attrs.is_empty()
            || call.args.len() != arity
            || !callee.attrs.is_empty()
            || callee.qself.is_some()
            || !method.arguments.is_none()
            || owner
                .segments
                .last()
                .is_none_or(|segment| !segment.arguments.is_none())
            || self.renamer.standard_collection(&owner) != Some(collection)
        {
            return false;
        }
        if let Some(mut arguments) = arguments {
            arguments.colon2_token = Some(syn::token::PathSep::default());
            let count = callee.path.segments.len();
            callee.path.segments[count - 2].arguments =
                syn::PathArguments::AngleBracketed(arguments);
        }
        true
    }

    fn scoped(&mut self, visit: impl FnOnce(&mut Self)) {
        self.scopes.push(BTreeMap::new());
        visit(self);
        self.scopes.pop();
    }

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

impl VisitMut for Fold<'_, '_> {
    fn visit_item_mut(&mut self, _: &mut syn::Item) {}

    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        self.scoped(|fold| {
            for stmt in &mut block.stmts {
                fold.visit_stmt_mut(stmt);
            }
        });
    }

    fn visit_local_mut(&mut self, local: &mut syn::Local) {
        let (pat, declared) = match &local.pat {
            syn::Pat::Type(typed) if local.attrs.is_empty() => {
                (typed.pat.as_ref().clone(), self.declared(&typed.ty))
            }
            pat => (pat.clone(), None),
        };
        let mut known = declared.clone();
        if let Some(init) = &mut local.init {
            self.visit_expr_mut(&mut init.expr);
            if let Some(target) = &declared {
                self.targeted(&mut init.expr, target);
            }
            if let Some((_, diverge)) = &mut init.diverge {
                self.visit_expr_mut(diverge);
            } else if !matches!(local.pat, syn::Pat::Type(_)) && local.attrs.is_empty() {
                known = self.source(&init.expr);
            }
        }
        self.annotation(local, declared.as_ref());
        self.bind(&pat, known);
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
            fold.bind(&arm.pat, None);
            if let syn::Pat::Guard(guard) = &mut arm.pat {
                fold.visit_expr_mut(&mut guard.guard);
            }
            fold.visit_expr_mut(&mut arm.body);
        });
    }

    fn visit_expr_for_loop_mut(&mut self, expr: &mut syn::ExprForLoop) {
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
        self.take(expr);
        syn::visit_mut::visit_expr_mut(self, expr);
        self.standard_default(expr);
    }
}

/// The owner and its generic arguments of the unattributed, argument-free call `Owner::name()`.
fn owner_call(
    call: &syn::ExprCall,
    name: &str,
) -> Option<(syn::Ident, Option<syn::AngleBracketedGenericArguments>)> {
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    let [owner, method] = path.path.segments.iter().collect::<Vec<_>>()[..] else {
        return None;
    };
    if !call.attrs.is_empty()
        || !call.args.is_empty()
        || !path.attrs.is_empty()
        || path.qself.is_some()
        || path.path.leading_colon.is_some()
        || !method.arguments.is_none()
        || method.ident != name
        || matches!(owner.arguments, syn::PathArguments::Parenthesized(_))
    {
        return None;
    }
    let arguments = match &owner.arguments {
        syn::PathArguments::AngleBracketed(arguments) => Some(arguments.clone()),
        _ => None,
    };
    Some((owner.ident.clone(), arguments))
}

/// Whether `call` is the unattributed, argument-free `Default::default()`.
fn default_call(call: &syn::ExprCall) -> bool {
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return false;
    };
    let [owner, method] = path.path.segments.iter().collect::<Vec<_>>()[..] else {
        return false;
    };
    call.attrs.is_empty()
        && call.args.is_empty()
        && path.attrs.is_empty()
        && path.qself.is_none()
        && path.path.leading_colon.is_none()
        && owner.arguments.is_none()
        && method.arguments.is_none()
        && owner.ident == "Default"
        && method.ident == "default"
}

/// The call `Owner::name(argument)`, with the owner's generic `arguments` kept.
fn associated_call(
    owner: syn::Ident,
    arguments: Option<syn::AngleBracketedGenericArguments>,
    name: &str,
    argument: Option<syn::Expr>,
) -> syn::Expr {
    let mut path = syn::Path::from(owner);
    if let (Some(arguments), Some(segment)) = (arguments, path.segments.last_mut()) {
        segment.arguments = syn::PathArguments::AngleBracketed(arguments);
    }
    path.segments.push(syn::PathSegment::from(ident(name)));
    syn::Expr::Call(syn::ExprCall {
        attrs: Vec::new(),
        func: Box::new(syn::Expr::Path(syn::ExprPath {
            attrs: Vec::new(),
            qself: None,
            path,
        })),
        paren_token: syn::token::Paren::default(),
        args: argument.into_iter().collect(),
    })
}

/// Whether `arguments` leave any type to inference with `_`.
fn infers(arguments: &syn::AngleBracketedGenericArguments) -> bool {
    struct Infer(bool);
    impl syn::visit::Visit<'_> for Infer {
        fn visit_type_infer(&mut self, _: &syn::TypeInfer) {
            self.0 = true;
        }
    }
    let mut infer = Infer(false);
    syn::visit::Visit::visit_angle_bracketed_generic_arguments(&mut infer, arguments);
    infer.0
}

fn ident(name: &str) -> syn::Ident {
    syn::Ident::new(name, proc_macro2::Span::call_site())
}

fn placeholder() -> syn::Expr {
    syn::Expr::Verbatim(proc_macro2::TokenStream::new())
}
