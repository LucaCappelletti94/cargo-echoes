//! The Rust edition a source context compiles under.

/// A Rust edition, as a manifest's `edition` key spells it.
///
/// Its methods answer the questions canonicalization asks of the edition.
///
/// ```
/// use syn_canon::Edition;
///
/// assert_eq!("2021".parse::<Edition>(), Ok(Edition::E2021));
/// assert!(!Edition::E2015.uses_extern_prelude());
/// assert!(Edition::E2024.uses_extern_prelude());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Edition {
    /// Rust 2015.
    E2015,
    /// Rust 2018.
    E2018,
    /// Rust 2021.
    E2021,
    /// Rust 2024.
    E2024,
}

impl Edition {
    /// Whether the first segment of a `use` path names a crate through the extern prelude.
    ///
    /// Rust 2015 resolves `use foo::Bar;` from the crate root instead.
    #[must_use]
    pub const fn uses_extern_prelude(self) -> bool {
        !matches!(self, Self::E2015)
    }
}

/// A manifest `edition` value that names no known edition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownEdition;

impl core::fmt::Display for UnknownEdition {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("unknown Rust edition")
    }
}

impl core::error::Error for UnknownEdition {}

impl core::str::FromStr for Edition {
    type Err = UnknownEdition;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "2015" => Ok(Self::E2015),
            "2018" => Ok(Self::E2018),
            "2021" => Ok(Self::E2021),
            "2024" => Ok(Self::E2024),
            _ => Err(UnknownEdition),
        }
    }
}
