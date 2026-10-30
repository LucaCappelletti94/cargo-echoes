//! Function-view relations for independently executed import path witnesses.

/// One independently executed import path source pair.
#[derive(serde::Deserialize)]
struct Case {
    id: String,
    a: String,
    b: String,
    expected: String,
}

/// The view of the function `f` in `source`, a crate root under `edition`.
fn view(source: &str, edition: syn_canon::Edition) -> syn_canon::CanonicalForm {
    let file = syn::parse_str::<syn::File>(source).unwrap();
    let context = syn_canon::SourceContext::new(core::iter::once((&[][..], &file)), edition);
    let function = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "f" => Some(function),
            _ => None,
        })
        .unwrap();
    context
        .function(&function.sig, &function.block)
        .unwrap()
        .canonicalize()
}

#[test]
fn import_path_witnesses_preserve_their_function_view_relations() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/dependency/import_paths_cases.json")).unwrap();
    for case in &cases {
        for edition in [syn_canon::Edition::E2021, syn_canon::Edition::E2024] {
            let (left, right) = (view(&case.a, edition), view(&case.b, edition));
            if case.expected == "same" {
                assert_eq!(left, right, "{}", case.id);
            } else {
                assert_ne!(left, right, "{}", case.id);
            }
        }
    }
}
