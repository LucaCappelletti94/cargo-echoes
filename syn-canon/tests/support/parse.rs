/// The file `source` parses to, a syntax error panicking with the source.
pub(super) fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}
