use super::parse::parse;

/// `left` and `right` canonicalize to one form exactly when `equal`, whether or not
/// canonicalization may assume the source compiles.
pub(super) fn compare(left: &str, right: &str, equal: bool) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        let a = canonicalize(parse(left));
        let b = canonicalize(parse(right));
        assert_eq!(a == b, equal, "{left}\n{right}\n{a:?}\n{b:?}");
    }
}
