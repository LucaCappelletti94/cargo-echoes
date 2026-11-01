/// The two sources differ as compiling forms and as failing forms.
pub(super) fn separate(left: &str, right: &str) {
    assert_ne!(
        syn_canon::canonicalize(syn::parse_str(left).unwrap()),
        syn_canon::canonicalize(syn::parse_str(right).unwrap()),
        "{left}\n{right}"
    );
    assert_ne!(
        syn_canon::canonicalize_failing(syn::parse_str(left).unwrap()),
        syn_canon::canonicalize_failing(syn::parse_str(right).unwrap())
    );
}
