//! Paths a scan leaves out, written as gitignore-style globs.

use alloc::format;
use alloc::string::{String, ToString};

/// One `exclude` pattern, limited to one repository by an `owner/repo:` prefix.
#[derive(Debug, Clone)]
pub(crate) struct Exclude {
    /// The `owner/repo` the pattern is limited to, lowercased.
    repository: Option<String>,
    /// The globs a matching root-relative path satisfies.
    globs: globset::GlobSet,
}

impl Exclude {
    /// Compile `pattern`.
    pub(crate) fn new(pattern: &str) -> Result<Self, crate::Error> {
        let invalid = |reason: String| crate::Error::InvalidExclude {
            pattern: pattern.to_string(),
            reason,
        };
        let (repository, glob) = match pattern.split_once(':') {
            Some((repository, glob)) if crate::organization_source::is_repository(repository) => {
                (Some(repository.to_ascii_lowercase()), glob)
            }
            _ => (None, pattern),
        };
        if glob.starts_with('!') {
            return Err(invalid("negates, which is not supported".to_string()));
        }
        let directory = glob.ends_with('/');
        let body = glob.trim_end_matches('/');
        // A slash before the end anchors the glob to the root, as in a `.gitignore`.
        let anchored = body.contains('/');
        let body = body.trim_start_matches('/');
        if body.is_empty() {
            return Err(invalid("names no path".to_string()));
        }
        let base = if anchored {
            body.to_string()
        } else {
            format!("**/{body}")
        };
        let mut set = globset::GlobSetBuilder::new();
        let contents = format!("{base}/**");
        let variants: &[&str] = if directory {
            &[&contents]
        } else {
            &[&base, &contents]
        };
        for variant in variants {
            set.add(
                globset::GlobBuilder::new(variant)
                    .literal_separator(true)
                    .backslash_escape(true)
                    .build()
                    .map_err(|err| invalid(err.kind().to_string()))?,
            );
        }
        Ok(Self {
            repository,
            globs: set.build().map_err(|err| invalid(err.to_string()))?,
        })
    }
}

/// Whether one of `excludes` leaves out the file at scan path `path`, which sits under `root`
/// in a workspace and starts with its `owner/repo` in an organization scan.
pub(crate) fn excluded(excludes: &[Exclude], organization: bool, root: &str, path: &str) -> bool {
    if excludes.is_empty() {
        return false;
    }
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .trim_start_matches(['/', '\\'])
        .replace('\\', "/");
    let (repository, relative) = if organization {
        // Every organization scan path starts with its `owner/repo`.
        let mut parts = relative.splitn(3, '/');
        let owner = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default();
        (
            Some(format!("{owner}/{name}")),
            parts.next().unwrap_or_default(),
        )
    } else {
        (None, relative.as_str())
    };
    excludes.iter().any(|exclude| {
        exclude.repository.as_deref().is_none_or(|limited| {
            repository
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(limited))
        }) && exclude.globs.is_match(relative)
    })
}
