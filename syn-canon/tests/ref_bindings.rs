//! A pattern whose every binding is `ref` reads as the binding-mode spelling over a borrow of
//! its place scrutinee.

fn parse(source: &str) -> syn::File {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn equivalent(explicit: &str, ergonomic: &str) {
    let left = syn_canon::canonicalize(parse(explicit));
    let right = syn_canon::canonicalize(parse(ergonomic));
    assert_eq!(left, right, "{explicit}\n{ergonomic}");
    assert_eq!(left.leaf_tokens(), right.leaf_tokens(), "{explicit}");
    assert_eq!(left.body_units(), right.body_units(), "{explicit}");
    assert_ne!(
        syn_canon::canonicalize_failing(parse(explicit)),
        syn_canon::canonicalize_failing(parse(ergonomic)),
        "{explicit}\n{ergonomic}"
    );
}

fn separate(left: &str, right: &str) {
    for canonicalize in [syn_canon::canonicalize, syn_canon::canonicalize_failing] {
        assert_ne!(
            canonicalize(parse(left)),
            canonicalize(parse(right)),
            "{left}\n{right}"
        );
    }
}

#[test]
fn ref_bindings_read_as_borrowed_scrutinees() {
    for (explicit, ergonomic) in [
        (
            "fn f(o: Option<String>) -> usize { match o { Some(ref s) => s.len(), None => 0 } }",
            "fn f(o: Option<String>) -> usize { match &o { Some(s) => s.len(), None => 0 } }",
        ),
        (
            "fn f(o: Option<String>) -> usize { if let Some(ref s) = o { s.len() } else { 0 } }",
            "fn f(o: Option<String>) -> usize { if let Some(s) = &o { s.len() } else { 0 } }",
        ),
        (
            "fn f(o: Option<u32>) -> u32 { let mut n = 0; while let Some(ref x) = o { n += *x; if n > 9 { break; } } n }",
            "fn f(o: Option<u32>) -> u32 { let mut n = 0; while let Some(x) = &o { n += *x; if n > 9 { break; } } n }",
        ),
        (
            "fn f(t: (String, String)) -> usize { let (ref a, ref b) = t; a.len() + b.len() }",
            "fn f(t: (String, String)) -> usize { let (a, b) = &t; a.len() + b.len() }",
        ),
        (
            "fn f(t: String) -> usize { let ref a = t; a.len() }",
            "fn f(t: String) -> usize { let a = &t; a.len() }",
        ),
        (
            "fn f(o: Option<String>) -> usize { let Some(ref s) = o else { return 0 }; s.len() }",
            "fn f(o: Option<String>) -> usize { let Some(s) = &o else { return 0 }; s.len() }",
        ),
        (
            "fn f(mut o: Option<String>) { if let Some(ref mut s) = o { s.push('a') } }",
            "fn f(mut o: Option<String>) { if let Some(s) = &mut o { s.push('a') } }",
        ),
        (
            "enum E { A { x: String, y: (u8, String) }, B }\nfn f(e: E) -> usize { match e { E::A { ref x, y: (_, ref z) } => x.len() + z.len(), E::B => 0 } }",
            "enum E { A { x: String, y: (u8, String) }, B }\nfn f(e: E) -> usize { match &e { E::A { x, y: (_, z) } => x.len() + z.len(), E::B => 0 } }",
        ),
        (
            "fn f(o: Option<String>) -> bool { match o { ref whole @ Some(_) => whole.is_some(), None => false } }",
            "fn f(o: Option<String>) -> bool { match &o { whole @ Some(_) => whole.is_some(), None => false } }",
        ),
        (
            "fn f(a: [String; 3]) -> usize { let [ref first, ref rest @ ..] = a; first.len() + rest.len() }",
            "fn f(a: [String; 3]) -> usize { let [first, rest @ ..] = &a; first.len() + rest.len() }",
        ),
        (
            "fn f(o: Option<String>) -> usize { if let Some(ref s) = o { s.len() } else { 0 } }",
            "fn f(o: Option<String>) -> usize { match &o { Some(s) => { s.len() } _ => 0 } }",
        ),
        (
            "fn f(o: Option<String>) -> usize { match o { Some(ref s) => { s.len() } _ => 0 } }",
            "fn f(o: Option<String>) -> usize { if let Some(s) = &o { s.len() } else { 0 } }",
        ),
    ] {
        equivalent(explicit, ergonomic);
    }
}

#[test]
fn dereferenced_shared_references_read_as_the_reference() {
    for (explicit, ergonomic) in [
        (
            "fn f(o: &Option<String>) -> usize { match *o { Some(ref s) => s.len(), None => 0 } }",
            "fn f(o: &Option<String>) -> usize { match o { Some(s) => s.len(), None => 0 } }",
        ),
        (
            "struct S { v: Option<String> }\nimpl S { fn f(&self) -> usize { match *self { S { v: Some(ref s) } => s.len(), _ => 0 } } }",
            "struct S { v: Option<String> }\nimpl S { fn f(&self) -> usize { match self { S { v: Some(s) } => s.len(), _ => 0 } } }",
        ),
        (
            "struct S { v: Option<String> }\nimpl S { fn f(&self) -> usize { match self.v { Some(ref s) => s.len(), None => 0 } } }",
            "struct S { v: Option<String> }\nimpl S { fn f(&self) -> usize { match &self.v { Some(s) => s.len(), None => 0 } } }",
        ),
    ] {
        equivalent(explicit, ergonomic);
    }
}

#[test]
fn moves_borrows_and_unproven_scrutinees_stay_apart() {
    for (left, right) in [
        (
            "fn f(t: (String, String)) -> usize { match t { (ref a, b) => a.len() + b.len() } }",
            "fn f(t: (String, String)) -> usize { match &t { (a, b) => a.len() + b.len() } }",
        ),
        (
            "fn f(o: &Option<String>) -> usize { match o { &Some(ref s) => s.len(), &None => 0 } }",
            "fn f(o: &Option<String>) -> usize { match &o { &Some(s) => s.len(), &None => 0 } }",
        ),
        (
            "fn f() -> usize { match g() { Some(ref s) => s.len(), None => 0 } }",
            "fn f() -> usize { match &g() { Some(s) => s.len(), None => 0 } }",
        ),
        (
            "fn f(o: Box<Option<String>>) -> usize { match *o { Some(ref s) => s.len(), None => 0 } }",
            "fn f(o: Box<Option<String>>) -> usize { match o { Some(s) => s.len(), None => 0 } }",
        ),
        (
            "fn f(o: Option<String>) -> usize { match o { Some(s) => s.len(), None => 0 } }",
            "fn f(o: Option<String>) -> usize { match &o { Some(s) => s.len(), None => 0 } }",
        ),
        (
            "fn f(o: Option<String>) -> usize { match o { Some(ref s) => s.len(), Missing => 0 } }",
            "fn f(o: Option<String>) -> usize { match &o { Some(s) => s.len(), Missing => 0 } }",
        ),
        (
            "fn f(o: &mut Option<String>) -> usize { match *o { Some(ref s) => s.len(), None => 0 } }",
            "fn f(o: &mut Option<String>) -> usize { match o { Some(s) => s.len(), None => 0 } }",
        ),
        (
            "fn f(o: &Option<String>) -> usize { let o = Box::new(o.clone()); match *o { Some(ref s) => s.len(), None => 0 } }",
            "fn f(o: &Option<String>) -> usize { let o = Box::new(o.clone()); match o { Some(s) => s.len(), None => 0 } }",
        ),
        (
            "fn f(o: Option<String>) -> usize { match o { Some(ref s) => s.len(), Some(ref t) if t.is_empty() => 1, None => 0 } }",
            "fn f(o: Option<String>) -> usize { match o { Some(s) => s.len(), Some(ref t) if t.is_empty() => 1, None => 0 } }",
        ),
        (
            "fn f(mut t: (String, String)) -> usize { match t { (ref a, ref mut b) => { b.push('a'); a.len() } } }",
            "fn f(mut t: (String, String)) -> usize { match &mut t { (a, b) => { b.push('a'); a.len() } } }",
        ),
    ] {
        separate(left, right);
    }
}
