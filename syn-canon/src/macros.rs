#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StandardRoot {
    Std,
    Core,
    Alloc,
}

impl StandardRoot {
    pub(crate) fn named(name: &str) -> Option<Self> {
        match crate::scope::unraw(name) {
            "std" => Some(Self::Std),
            "core" => Some(Self::Core),
            "alloc" => Some(Self::Alloc),
            _ => None,
        }
    }

    pub(crate) fn grammar(self, name: &str) -> Option<MacroGrammar> {
        let name = crate::scope::unraw(name);
        if !crate::COMMA_BLIND_MACROS.contains(&name)
            || match self {
                Self::Std => false,
                Self::Alloc => !matches!(name, "format" | "vec"),
                Self::Core => matches!(
                    name,
                    "dbg" | "eprint" | "eprintln" | "format" | "print" | "println" | "vec"
                ),
            }
        {
            return None;
        }
        Some(match name {
            "vec" => MacroGrammar::Vector,
            "matches" => MacroGrammar::Matches,
            _ => MacroGrammar::Expressions,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MacroGrammar {
    Expressions,
    Vector,
    Matches,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MacroResolution {
    Crate(StandardRoot),
    Macro(MacroGrammar),
    Opaque,
}

impl MacroResolution {
    pub(crate) fn external_path(path: &[alloc::string::String]) -> Option<Self> {
        match path {
            [root] => StandardRoot::named(root).map(Self::Crate),
            [root, name] if name == "self" => StandardRoot::named(root).map(Self::Crate),
            [root, name] => StandardRoot::named(root)?.grammar(name).map(Self::Macro),
            _ => None,
        }
    }

    pub(crate) fn external_target(target: &crate::scope::Target) -> Option<Self> {
        match target.segments.as_slice() {
            [root] if root.origin.is_none() => StandardRoot::named(&root.name).map(Self::Crate),
            [root, name] if root.origin.is_none() && name.origin.is_none() => {
                StandardRoot::named(&root.name)?
                    .grammar(&name.name)
                    .map(Self::Macro)
            }
            _ => None,
        }
    }

    pub(crate) fn external_crate(item: &syn::ItemExternCrate) -> Option<Self> {
        if crate::context::blocking_attr(&item.attrs) {
            return None;
        }
        StandardRoot::named(&crate::scope::ident_name(&item.ident)).map(Self::Crate)
    }
}

pub(crate) struct MatchesArgs {
    pub(crate) value: syn::Expr,
    separator: syn::Token![,],
    pub(crate) pattern: syn::Pat,
    pub(crate) guard: Option<(syn::Token![if], syn::Expr)>,
    pub(crate) trailing: Option<syn::Token![,]>,
}

impl syn::parse::Parse for MatchesArgs {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            value: input.parse()?,
            separator: input.parse()?,
            pattern: input.call(syn::Pat::parse_multi_with_leading_vert)?,
            guard: if input.peek(syn::Token![if]) {
                Some((input.parse()?, input.parse()?))
            } else {
                None
            },
            trailing: input.parse()?,
        })
    }
}

impl quote::ToTokens for MatchesArgs {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.value.to_tokens(tokens);
        self.separator.to_tokens(tokens);
        self.pattern.to_tokens(tokens);
        if let Some((keyword, guard)) = &self.guard {
            keyword.to_tokens(tokens);
            guard.to_tokens(tokens);
        }
        self.trailing.to_tokens(tokens);
    }
}
