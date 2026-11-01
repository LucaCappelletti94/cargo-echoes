use syn_canon::CanonicalForm;

/// The canonical view of one parsed source, with opaque macro bodies.
pub(super) fn failing(source: &str) -> CanonicalForm {
    syn_canon::canonicalize_failing(syn::parse_str(source).unwrap())
}
