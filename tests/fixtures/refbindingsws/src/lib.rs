/// ```
/// fn size(o: Option<String>) -> usize {
///     match o { Some(ref s) => s.len(), None => 0 }
/// }
/// assert_eq!(size(Some(String::from("ab"))), 2);
/// ```
pub struct Explicit;

/// ```
/// fn size(o: Option<String>) -> usize {
///     match &o { Some(s) => s.len(), None => 0 }
/// }
/// assert_eq!(size(Some(String::from("ab"))), 2);
/// ```
pub struct Ergonomic;

/// ```
/// fn size(o: Option<String>) -> usize {
///     match o { Some(s) => s.len(), None => 0 }
/// }
/// assert_eq!(size(Some(String::from("ab"))), 2);
/// ```
pub struct Moved;

pub struct Config {
    pub name: Option<String>,
}

impl Config {
    pub fn explicit(&self) -> usize {
        match *self {
            Config { name: Some(ref name) } => name.len(),
            _ => 0,
        }
    }

    pub fn ergonomic(&self) -> usize {
        match self {
            Config { name: Some(name) } => name.len(),
            _ => 0,
        }
    }
}

pub fn label(pair: (String, u8)) -> usize {
    let (ref text, _) = pair;
    text.len()
}

pub fn borrowed(pair: (String, u8)) -> usize {
    let (text, _) = &pair;
    text.len()
}

pub fn moved(pair: (String, u8)) -> usize {
    let (text, _) = pair;
    text.len()
}
