/// The function `f` taking `params`, returning `ret` and running `body`.
pub(super) fn function(params: &str, ret: &str, body: &str) -> String {
    format!("fn f({params}) -> {ret} {{ {body} }}")
}
