use super::canon::equivalent;
use super::function::function;

/// Every spelling after the first of each group reads as the first, as the body of a function
/// taking `params` and returning `ret`.
pub(super) fn rows(params: &str, ret: &str, groups: &[&[&str]]) {
    for group in groups {
        for written in &group[1..] {
            equivalent(
                &function(params, ret, written),
                &function(params, ret, group[0]),
            );
        }
    }
}
