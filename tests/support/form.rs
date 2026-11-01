use syn_canon::CanonicalForm;

/// The canonical view of one parsed source.
pub(super) fn form(source: &str) -> CanonicalForm {
    syn_canon::canonicalize(syn::parse_str(source).unwrap())
}
