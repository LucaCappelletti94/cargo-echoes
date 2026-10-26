//! End-to-end tests of the scan through the real workspace fixtures.

#![cfg(feature = "std")]

use echoes::{ContextKind, Echoes, Remedy, Report};
use std::path::Path;
#[cfg(feature = "cli")]
use std::process::Command;

const FIXTURE: &str = "tests/fixtures/dupws";

fn scan() -> Report {
    echoes::Echoes::default().run(FIXTURE).unwrap()
}

const FN_FIXTURE: &str = "tests/fixtures/fnws";

/// A function group's items, self types and remedy.
type FnGroup<'a> = (Vec<&'a str>, Vec<Option<&'a str>>, Option<Remedy>);

#[test]
fn functions_group_per_module_across_targets_and_cfgs() {
    let report = Echoes::default().functions().run(FN_FIXTURE).unwrap();
    assert_eq!(
        (report.total, report.functions, report.unique_functions),
        (0, 17, 8)
    );
    let mut groups: Vec<FnGroup<'_>> = report
        .groups
        .iter()
        .map(|g| {
            assert_eq!(g.kind, echoes::Kind::Function);
            let items = g.sites.iter().map(|s| s.item.as_str()).collect();
            let types = g.sites.iter().map(|s| s.self_type.as_deref()).collect();
            (items, types, g.remedy)
        })
        .collect();
    groups.sort();
    assert_eq!(
        groups,
        [
            (
                vec![
                    "fnws::describe::Gamma::short",
                    "fnws::describe::Gamma::long"
                ],
                vec![Some("Gamma"), Some("Gamma")],
                Some(Remedy::HelperOrMacro)
            ),
            (
                vec!["fnws::platform::width", "fnws::platform::height"],
                vec![None, None],
                Some(Remedy::Delete)
            ),
            (
                vec!["fnws::render::Alpha::render", "fnws::render::Beta::render"],
                vec![Some("Alpha"), Some("Beta")],
                Some(Remedy::GenericOrMacro)
            ),
            (
                vec!["fnws::scale::scaled", "fnws::scale::scaled_elsewhere"],
                vec![None, None],
                Some(Remedy::MergeCfg)
            ),
            (
                vec![
                    "fnws::tests::totals_small_values",
                    "fnws::tests::totals_large_values"
                ],
                vec![None, None],
                Some(Remedy::Delete)
            ),
            (
                vec!["fnws::total_over", "fnws::sum_over", "fnws::total_too"],
                vec![None, None, None],
                Some(Remedy::Delete)
            ),
            (
                vec!["it::renders_values", "it::renders_values_again"],
                vec![None, None],
                Some(Remedy::Delete)
            ),
        ]
    );
    // A `pub` function of the library counts as public API whatever its module, since a
    // re-export can expose it. The integration test target has none.
    let mut public: Vec<&str> = report
        .groups
        .iter()
        .flat_map(|g| &g.sites)
        .filter(|s| s.public)
        .map(|s| s.item.as_str())
        .collect();
    public.sort_unstable();
    assert_eq!(
        public,
        [
            "fnws::platform::height",
            "fnws::platform::width",
            "fnws::render::Alpha::render",
            "fnws::render::Beta::render",
            "fnws::scale::scaled",
            "fnws::scale::scaled_elsewhere",
            "fnws::total_over",
            "fnws::total_too"
        ]
    );
    // A site runs from its first doc comment or attribute to its closing brace.
    for site in report.groups.iter().flat_map(|g| &g.sites) {
        let text = std::fs::read_to_string(Path::new(FN_FIXTURE).join(&site.file)).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let first = lines[site.line as usize - 1].trim_start();
        let last = lines[site.end.unwrap() as usize - 1].trim();
        assert!(
            first.starts_with("///") || first.starts_with("#[") || first.contains("fn "),
            "{site:?}"
        );
        assert_eq!(last, "}", "{site:?}");
        assert_eq!(
            site.code.lines().count(),
            (site.end.unwrap() - site.line + 1) as usize
        );
    }
    let off = Echoes::default().run(FN_FIXTURE).unwrap();
    assert_eq!((off.functions, off.groups.len()), (0, 0));
}

#[test]
fn copies_selected_by_module_declarations_merge_their_cfgs() {
    let report = Echoes::default()
        .functions()
        .run("tests/fixtures/modcfgws")
        .unwrap();
    let mut groups: Vec<(Vec<&str>, Vec<&str>, Option<Remedy>)> = report
        .groups
        .iter()
        .map(|g| {
            let items = g.sites.iter().map(|s| s.item.as_str()).collect();
            let files = g.sites.iter().map(|s| s.file.as_str()).collect();
            (items, files, g.remedy)
        })
        .collect();
    groups.sort();
    assert_eq!(
        groups,
        [
            (
                vec!["modcfgws::bits::spread", "modcfgws::bits::spread"],
                vec!["src/narrow/bits.rs", "src/wide/bits.rs"],
                Some(Remedy::MergeCfg)
            ),
            (
                vec![
                    "modcfgws::host::probe::measure",
                    "modcfgws::host::probe::measure"
                ],
                vec!["src/host/other_probe.rs", "src/host/probe.rs"],
                Some(Remedy::MergeCfg)
            ),
            (
                vec!["modcfgws::width::width", "modcfgws::width::width"],
                vec!["src/fast/width.rs", "src/slow/width.rs"],
                Some(Remedy::MergeCfg)
            ),
        ]
    );
}

#[test]
fn cfg_attr_branch_files_are_reported_and_never_linked_files_are_not() {
    let files = |report: &Report| -> Vec<String> {
        let mut files: Vec<String> = report
            .groups
            .iter()
            .flat_map(|g| g.sites.iter().map(|s| s.file.clone()))
            .chain(
                report
                    .context_groups
                    .iter()
                    .flat_map(|g| g.sites.iter().map(|s| s.file.clone())),
            )
            .filter(|file| file.contains("width.rs") || file.contains("tail.rs"))
            .collect();
        files.sort();
        files.dedup();
        files
    };
    let default = Echoes::default()
        .functions()
        .context_blocks()
        .run("tests/fixtures/modcfgws")
        .unwrap();
    assert_eq!(files(&default), ["src/fast/width.rs", "src/slow/width.rs"]);
    let cache = tempfile::tempdir().unwrap();
    let dependencies = Echoes::default()
        .context_blocks()
        .context_dependencies(echoes::DependencySearch {
            depth: Some(1),
            cache_dir: Some(cache.path().to_string_lossy().into_owned()),
            check_latest: Some(false),
            ..echoes::DependencySearch::default()
        })
        .run("tests/fixtures/modcfgws")
        .unwrap();
    assert_eq!(
        dependencies.context_groups, default.context_groups,
        "dependency mode reports the default local groups"
    );
}

#[test]
fn shared_group_spans_crates_and_variants() {
    let report = scan();
    assert_eq!(report.total, 9);
    assert_eq!(report.unique, 4);
    assert_eq!(report.groups.len(), 2);
    let shared = report
        .groups
        .iter()
        .find(|g| g.sites.iter().any(|s| s.item == "alpha::alpha_fn"))
        .expect("shared group present");
    assert_eq!(shared.sites.len(), 4);
    let items: Vec<&str> = shared.sites.iter().map(|s| s.item.as_str()).collect();
    // The `helper` copies name their item in the code and come first.
    assert_eq!(
        items,
        vec![
            "alpha::util::helper",
            "beta::util::helper",
            "alpha::alpha_fn",
            "alpha::alpha_variant"
        ]
    );
    let files: Vec<&str> = shared.sites.iter().map(|s| s.file.as_str()).collect();
    assert_eq!(
        files,
        vec![
            "alpha/src/util.rs",
            "beta/src/util.rs",
            "alpha/src/lib.rs",
            "alpha/src/lib.rs"
        ]
    );
}

#[test]
fn unparsed_pair_groups_with_flag() {
    let report = scan();
    let unparsed = report
        .groups
        .iter()
        .find(|g| g.unparsed)
        .expect("unparsed group present");
    assert_eq!(unparsed.sites.len(), 2);
}

#[test]
fn allowed_sites_are_not_grouped() {
    for group in scan().groups {
        for site in group.sites {
            assert!(!site.allow);
            assert_ne!(site.item, "alpha::alpha_allowed");
        }
    }
}

#[test]
fn config_threshold_suppresses_groups() {
    let strict = Path::new(FIXTURE).join(".echoes-strict.toml");
    let report = Echoes::default()
        .config(&strict)
        .unwrap()
        .run(FIXTURE)
        .unwrap();
    assert_eq!(report.groups, Vec::new());

    // API threshold overrides the config file.
    let report = Echoes::default()
        .threshold(2)
        .config(&strict)
        .unwrap()
        .run(FIXTURE)
        .unwrap();
    assert_eq!(report.groups.len(), 2);

    // The API threshold still wins when set after the config file.
    let report = Echoes::default()
        .config(&strict)
        .unwrap()
        .threshold(2)
        .run(FIXTURE)
        .unwrap();
    assert_eq!(report.groups.len(), 2);
}

#[test]
fn all_targets_builder_scans_bin_targets() {
    // The fixture's bin target (alpha/src/bin/dupbin.rs) is scanned only
    // with all_targets: one more doctest site, which alpha-renaming
    // joins into the function group.
    let report = Echoes::default().all_targets().run(FIXTURE).unwrap();
    assert_eq!(report.total, 10);
    assert_eq!(report.unique, 4);
    assert_eq!(report.groups.len(), 2);
}

#[test]
fn a_package_filter_keeps_the_package_bin_targets() {
    // `dupbin` is a target of package `alpha` with its own name.
    let report = Echoes::default()
        .package("alpha")
        .all_targets()
        .run(FIXTURE)
        .unwrap();
    assert!(
        report
            .groups
            .iter()
            .any(|g| g.sites.iter().any(|s| s.file.ends_with("dupbin.rs")))
    );
}

#[test]
fn min_tokens_builder_filters_blocks() {
    // min-tokens 4 drops the 3-token unparsed group; only the
    // 4-token function group survives.
    let report = Echoes::default().min_tokens(4).run(FIXTURE).unwrap();
    assert_eq!(report.total, 9);
    assert_eq!(report.groups.len(), 1);
}

#[test]
fn exit_code_reflects_duplicates() {
    let report = scan();
    assert_eq!(
        echoes::exit_code(&report, false),
        std::process::ExitCode::from(1)
    );
    assert_eq!(
        echoes::exit_code(&report, true),
        std::process::ExitCode::SUCCESS
    );
}
#[test]
#[cfg(feature = "cli")]
fn binary_runs_end_to_end() {
    let bin = env!("CARGO_BIN_EXE_cargo-echoes");
    let out = Command::new(bin)
        .current_dir(FIXTURE)
        .arg("echoes")
        .arg("--json")
        .output()
        .expect("run echoes binary");
    assert_eq!(out.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json output");
    assert_eq!(value["total"], 9);
    assert_eq!(value["groups"].as_array().unwrap().len(), 2);
}

#[test]
#[cfg(feature = "cli")]
fn an_excluded_member_leaves_the_scan() {
    let bin = env!("CARGO_BIN_EXE_cargo-echoes");
    let files = |extra: &[&str]| {
        let out = Command::new(bin)
            .current_dir(FIXTURE)
            .args(["echoes", "--json", "--no-fail"])
            .args(extra)
            .output()
            .expect("run echoes binary");
        assert_eq!(out.status.code(), Some(0), "{extra:?}");
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let mut files: Vec<String> = value["groups"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|group| group["sites"].as_array().unwrap().iter())
            .map(|site| site["file"].as_str().unwrap().to_string())
            .collect();
        files.sort();
        files.dedup();
        files
    };
    assert!(files(&[]).iter().any(|file| file.starts_with("beta/")));
    let kept = files(&["--exclude", "beta/", "--exclude", "*.md"]);
    assert!(
        kept.iter().all(|file| !file.starts_with("beta/")),
        "{kept:?}"
    );
}

#[test]
#[cfg(feature = "cli")]
fn binary_prints_annotations_beside_the_report() {
    let bin = env!("CARGO_BIN_EXE_cargo-echoes");
    let out = Command::new(bin)
        .current_dir(FIXTURE)
        .arg("echoes")
        .arg("--github")
        .output()
        .expect("run echoes binary");
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).expect("utf8 output");
    assert!(text.contains("9 doctests (4 unique)"), "{text}");
    let marks: Vec<&str> = text.lines().filter(|l| l.starts_with("::")).collect();
    assert_eq!(marks.len(), 4, "{text}");
    assert!(
        marks
            .iter()
            .all(|l| l.starts_with("::error file=") && l.contains("title=echoes::")),
        "{text}"
    );
}

#[test]
#[cfg(feature = "cli")]
fn unknown_package_fails_with_its_name() {
    // Only the cargo subcommand token at position 1 is stripped, so a flag value spelled
    // "echoes" reaches workspace discovery, which names no such member.
    let bin = env!("CARGO_BIN_EXE_cargo-echoes");
    let out = Command::new(bin)
        .current_dir(FIXTURE)
        .arg("echoes")
        .arg("--json")
        .arg("--package")
        .arg("echoes")
        .output()
        .expect("run echoes binary");
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no workspace member named `echoes`"),
        "{stderr}"
    );
}

#[test]
#[cfg(feature = "cli")]
fn a_bare_organization_flag_scans_the_current_repository_owner() {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .current_dir(dir.path())
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&[
        "remote",
        "add",
        "origin",
        "https://gitlab.com/owner/app.git",
    ]);
    let bin = env!("CARGO_BIN_EXE_cargo-echoes");
    let out = Command::new(bin)
        .current_dir(dir.path())
        .env_remove("GITHUB_REPOSITORY_OWNER")
        .args(["echoes", "--organization", "--no-fail"])
        .output()
        .expect("run echoes binary");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("is not a GitHub repository"), "{stderr}");
    // An invalid owner is refused before any request, so no network is needed.
    for args in [
        ["echoes", "--organization", "--no-fail", "--json"],
        ["echoes", "--organization=../evil", "--no-fail", "--json"],
    ] {
        let out = Command::new(bin)
            .current_dir(dir.path())
            .env("GITHUB_REPOSITORY_OWNER", "../evil")
            .args(args)
            .output()
            .expect("run echoes binary");
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(report["organization"]["name"], "../evil", "{args:?}");
    }
}

#[test]
#[cfg(feature = "cli")]
fn organizations_and_repositories_join_one_selection() {
    let dir = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_cargo-echoes");
    // Malformed names are refused before any request, so no network is needed.
    let out = Command::new(bin)
        .current_dir(dir.path())
        .args([
            "echoes",
            "--organization=-bad",
            "--repository",
            "app",
            "--repository",
            "o/../x",
            "--no-fail",
            "--json",
        ])
        .output()
        .expect("run echoes binary");
    assert_eq!(out.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        report["organization"]["coverage"]["failures"],
        serde_json::json!([
            "-bad: the organization login is not a valid GitHub login",
            "app: is not an owner/repository name",
            "o/../x: is not an owner/repository name"
        ])
    );
}

#[test]
fn file_module_items_carry_module_prefix() {
    // Items in file modules carry their module path, as rustdoc names
    // them (`util::helper`, not `helper`).
    let report = scan();
    let helper = report
        .groups
        .iter()
        .find(|g| g.sites.iter().any(|s| s.item.contains("helper")))
        .expect("helper group present");
    let items: Vec<&str> = helper
        .sites
        .iter()
        .map(|s| s.item.as_str())
        .filter(|item| item.contains("helper"))
        .collect();
    assert_eq!(items, vec!["alpha::util::helper", "beta::util::helper"]);
}

#[test]
fn threshold_builder_narrows_groups() {
    // Dupws has one 4-site group and one 2-site group: raising the
    // threshold through the builder keeps only the 4-site group.
    let report = echoes::Echoes::default().threshold(3).run(FIXTURE).unwrap();
    assert_eq!(report.total, 9);
    assert_eq!(report.groups.len(), 1);
}

const MACROVEC_FIXTURE: &str = "tests/fixtures/macrovecws";

#[test]
fn qualified_custom_macros_keep_their_comma_in_doctests() {
    let report = Echoes::default().run(MACROVEC_FIXTURE).unwrap();
    assert_eq!((report.total, report.unique), (4, 3));
    assert_eq!(report.groups.len(), 1);
    let group = &report.groups[0];
    assert_eq!(group.kind, echoes::Kind::Doctest);
    let items: Vec<&str> = group.sites.iter().map(|s| s.item.as_str()).collect();
    assert_eq!(
        items,
        vec!["macrovecws::Standard", "macrovecws::StandardComma"]
    );
}

#[test]
fn qualified_custom_macros_keep_their_comma_in_functions() {
    let report = Echoes::default()
        .functions()
        .fn_min_tokens(0)
        .run(MACROVEC_FIXTURE)
        .unwrap();
    assert_eq!((report.functions, report.unique_functions), (4, 3));
    let function_groups = report
        .groups
        .iter()
        .filter(|g| g.kind == echoes::Kind::Function)
        .collect::<Vec<_>>();
    assert_eq!(function_groups.len(), 1);
    let group = function_groups[0];
    let items: Vec<&str> = group.sites.iter().map(|s| s.item.as_str()).collect();
    assert_eq!(
        items,
        vec!["macrovecws::equal_first", "macrovecws::equal_second"]
    );
    assert_eq!(group.remedy, Some(Remedy::Delete));
}

const TAILRETURN_FIXTURE: &str = "tests/fixtures/tailreturnws";

#[test]
fn destructor_sensitive_tail_returns_do_not_group_in_doctests() {
    let report = Echoes::default().run(TAILRETURN_FIXTURE).unwrap();
    assert_eq!((report.total, report.unique), (4, 3));
    assert_eq!(report.groups.len(), 1);
    let group = &report.groups[0];
    assert_eq!(group.kind, echoes::Kind::Doctest);
    let items: Vec<&str> = group.sites.iter().map(|s| s.item.as_str()).collect();
    assert_eq!(
        items,
        vec!["tailreturnws::PlainReturn", "tailreturnws::PlainTail"]
    );
    for candidate in &report.groups {
        let items: Vec<&str> = candidate.sites.iter().map(|s| s.item.as_str()).collect();
        assert!(
            !items.contains(&"tailreturnws::GuardReturn")
                || !items.contains(&"tailreturnws::GuardTail"),
            "the destructor-sensitive return and tail forms must not group"
        );
    }
}

#[test]
fn destructor_sensitive_tail_returns_do_not_group_in_functions() {
    let report = Echoes::default()
        .functions()
        .fn_min_tokens(0)
        .run(TAILRETURN_FIXTURE)
        .unwrap();
    assert_eq!((report.functions, report.unique_functions), (6, 5));
    let function_groups = report
        .groups
        .iter()
        .filter(|g| g.kind == echoes::Kind::Function)
        .collect::<Vec<_>>();
    assert_eq!(function_groups.len(), 1);
    let group = function_groups[0];
    let items: Vec<&str> = group.sites.iter().map(|s| s.item.as_str()).collect();
    assert_eq!(
        items,
        vec!["tailreturnws::equal_first", "tailreturnws::equal_second"]
    );
    assert_eq!(group.remedy, Some(Remedy::Delete));
}

const REFLEXIVE_FIXTURE: &str = "tests/fixtures/reflexivews";

#[test]
fn reflexive_doctests_group_only_proven_primitive_comparisons() {
    let report = Echoes::default().run(REFLEXIVE_FIXTURE).unwrap();
    let mut groups: Vec<Vec<&str>> = report
        .groups
        .iter()
        .map(|group| {
            assert_eq!(group.kind, echoes::Kind::Doctest);
            group.sites.iter().map(|site| site.item.as_str()).collect()
        })
        .collect();
    for group in &mut groups {
        group.sort_unstable();
    }
    assert_eq!(
        groups,
        [vec!["reflexivews::Folded", "reflexivews::Written"]]
    );
}

#[test]
fn reflexive_functions_keep_float_partial_and_custom_equality_separate() {
    let report = Echoes::default()
        .functions()
        .fn_min_tokens(0)
        .run(REFLEXIVE_FIXTURE)
        .unwrap();
    let mut groups: Vec<Vec<&str>> = report
        .groups
        .iter()
        .filter(|group| group.kind == echoes::Kind::Function)
        .map(|group| group.sites.iter().map(|site| site.item.as_str()).collect())
        .collect();
    for group in &mut groups {
        group.sort_unstable();
    }
    assert_eq!(
        groups,
        [vec!["reflexivews::folded", "reflexivews::written"]]
    );
}

const XOR_FIXTURE: &str = "tests/fixtures/xorws";

/// The function groups of a report, member-sorted and group-sorted.
fn function_groups(report: &Report) -> Vec<Vec<&str>> {
    let mut groups: Vec<Vec<&str>> = report
        .groups
        .iter()
        .filter(|group| group.kind == echoes::Kind::Function)
        .map(|group| group.sites.iter().map(|site| site.item.as_str()).collect())
        .collect();
    for group in &mut groups {
        group.sort_unstable();
    }
    groups.sort_unstable();
    groups
}

#[test]
fn xor_doctests_group_only_the_proven_primitive_cancellations() {
    let report = Echoes::default().run(XOR_FIXTURE).unwrap();
    assert_eq!(report.total, 16);
    assert_eq!(report.unique, 11);
    let mut groups: Vec<Vec<&str>> = report
        .groups
        .iter()
        .map(|group| {
            assert_eq!(group.kind, echoes::Kind::Doctest);
            group.sites.iter().map(|site| site.item.as_str()).collect()
        })
        .collect();
    for group in &mut groups {
        group.sort_unstable();
    }
    groups.sort_unstable();
    assert_eq!(
        groups,
        [
            vec!["xorws::BoolFalse", "xorws::BoolWritten"],
            vec!["xorws::Chain", "xorws::Remaining"],
            vec![
                "xorws::LiteralIdentity",
                "xorws::OddWritten",
                "xorws::Plain"
            ],
            vec!["xorws::Written", "xorws::Zero"],
        ]
    );
}

#[test]
fn xor_doctest_groups_carry_their_normalized_token_size() {
    let report = Echoes::default().run(XOR_FIXTURE).unwrap();
    for group in &report.groups {
        assert_eq!(
            group.tokens,
            if group.sites.iter().any(|site| site.item == "xorws::Chain") {
                15
            } else {
                11
            }
        );
    }
    let kept = Echoes::default().min_tokens(12).run(XOR_FIXTURE).unwrap();
    let mut groups: Vec<Vec<&str>> = kept
        .groups
        .iter()
        .map(|group| group.sites.iter().map(|site| site.item.as_str()).collect())
        .collect();
    for group in &mut groups {
        group.sort_unstable();
    }
    groups.sort_unstable();
    assert_eq!(groups, [vec!["xorws::Chain", "xorws::Remaining"]]);
    assert_eq!(
        Echoes::default()
            .min_tokens(16)
            .run(XOR_FIXTURE)
            .unwrap()
            .groups
            .len(),
        0
    );
}

#[test]
fn xor_functions_keep_the_unproven_cancellations_separate() {
    let report = Echoes::default()
        .functions()
        .fn_min_tokens(0)
        .run(XOR_FIXTURE)
        .unwrap();
    assert_eq!(
        function_groups(&report),
        [
            vec!["xorws::boolean::folded", "xorws::boolean::written"],
            vec!["xorws::chain", "xorws::remaining"],
            vec![
                "xorws::literal_identity",
                "xorws::odd_written",
                "xorws::plain"
            ],
            vec!["xorws::written", "xorws::zero"],
        ]
    );
}

#[test]
fn xor_function_groups_carry_their_normalized_body_size() {
    let kept = Echoes::default()
        .functions()
        .fn_min_tokens(1)
        .run(XOR_FIXTURE)
        .unwrap();
    assert_eq!(
        function_groups(&kept),
        [
            vec!["xorws::boolean::folded", "xorws::boolean::written"],
            vec!["xorws::chain", "xorws::remaining"],
            vec![
                "xorws::literal_identity",
                "xorws::odd_written",
                "xorws::plain"
            ],
            vec!["xorws::written", "xorws::zero"],
        ]
    );
    assert!(
        kept.groups
            .iter()
            .filter(|group| group.kind == echoes::Kind::Function)
            .all(|group| group.tokens == 1)
    );
    let dropped = Echoes::default()
        .functions()
        .fn_min_tokens(2)
        .run(XOR_FIXTURE)
        .unwrap();
    assert!(
        dropped
            .groups
            .iter()
            .all(|group| group.kind != echoes::Kind::Function)
    );
}

const WRAPPING_FIXTURE: &str = "tests/fixtures/wrappingws";

/// The doctest groups of a report, member-sorted and group-sorted.
fn doctest_groups(report: &Report) -> Vec<Vec<&str>> {
    let mut groups: Vec<Vec<&str>> = report
        .groups
        .iter()
        .filter(|group| group.kind == echoes::Kind::Doctest)
        .map(|group| group.sites.iter().map(|site| site.item.as_str()).collect())
        .collect();
    for group in &mut groups {
        group.sort_unstable();
    }
    groups.sort_unstable();
    groups
}

#[test]
fn wrapping_add_doctests_group_only_the_proven_native_operand_orders() {
    let report = Echoes::default().run(WRAPPING_FIXTURE).unwrap();
    assert_eq!(report.total, 14);
    assert_eq!(report.unique, 11);
    assert!(
        report
            .groups
            .iter()
            .all(|group| group.kind == echoes::Kind::Doctest)
    );
    assert_eq!(
        doctest_groups(&report),
        [
            vec!["wrappingws::LiteralReordered", "wrappingws::LiteralWritten"],
            vec!["wrappingws::Reordered", "wrappingws::Written"],
            vec!["wrappingws::SignedReordered", "wrappingws::SignedWritten"],
        ]
    );
}

#[test]
fn wrapping_add_doctest_groups_carry_their_source_token_size() {
    let report = Echoes::default().run(WRAPPING_FIXTURE).unwrap();
    for group in &report.groups {
        let literal = group
            .sites
            .iter()
            .any(|site| site.item == "wrappingws::LiteralWritten");
        assert_eq!(group.tokens, if literal { 14 } else { 18 });
    }
    let kept = Echoes::default()
        .min_tokens(15)
        .run(WRAPPING_FIXTURE)
        .unwrap();
    assert_eq!(
        doctest_groups(&kept),
        [
            vec!["wrappingws::Reordered", "wrappingws::Written"],
            vec!["wrappingws::SignedReordered", "wrappingws::SignedWritten"],
        ]
    );
    let dropped = Echoes::default()
        .min_tokens(19)
        .run(WRAPPING_FIXTURE)
        .unwrap();
    assert_eq!(dropped.groups.len(), 0);
}

#[test]
fn wrapping_add_functions_keep_custom_effectful_and_other_operations_separate() {
    let report = Echoes::default()
        .functions()
        .fn_min_tokens(0)
        .run(WRAPPING_FIXTURE)
        .unwrap();
    let expected = [
        vec![
            "wrappingws::literal_reordered",
            "wrappingws::literal_written",
        ],
        vec!["wrappingws::reordered", "wrappingws::written"],
        vec![
            "wrappingws::signed::reordered",
            "wrappingws::signed::written",
        ],
    ];
    assert_eq!(function_groups(&report), expected);
    let kept = Echoes::default()
        .functions()
        .fn_min_tokens(4)
        .run(WRAPPING_FIXTURE)
        .unwrap();
    assert_eq!(function_groups(&kept), expected);
    let dropped = Echoes::default()
        .functions()
        .fn_min_tokens(5)
        .run(WRAPPING_FIXTURE)
        .unwrap();
    assert_eq!(function_groups(&dropped).len(), 0);
}

const BOOLEAN_MATCH_FIXTURE: &str = "tests/fixtures/boolmatchws";

#[test]
fn boolean_match_doctests_group_only_the_proven_conditional_forms() {
    let report = Echoes::default().run(BOOLEAN_MATCH_FIXTURE).unwrap();
    assert_eq!(report.total, 8);
    assert_eq!(report.unique, 6);
    assert_eq!(
        doctest_groups(&report),
        [vec![
            "boolmatchws::Conditional",
            "boolmatchws::Matched",
            "boolmatchws::Reversed",
        ]]
    );
    let group = &report.groups[0];
    let kept = Echoes::default()
        .min_tokens(group.tokens)
        .run(BOOLEAN_MATCH_FIXTURE)
        .unwrap();
    assert_eq!(doctest_groups(&kept), doctest_groups(&report));
    let dropped = Echoes::default()
        .min_tokens(group.tokens + 1)
        .run(BOOLEAN_MATCH_FIXTURE)
        .unwrap();
    assert_eq!(dropped.groups.len(), 0);
}

#[test]
fn boolean_match_functions_keep_wildcards_and_effects_separate() {
    let expected = [vec![
        "boolmatchws::conditional",
        "boolmatchws::matched",
        "boolmatchws::reversed",
    ]];
    for tokens in [0, 5] {
        let report = Echoes::default()
            .functions()
            .fn_min_tokens(tokens)
            .run(BOOLEAN_MATCH_FIXTURE)
            .unwrap();
        assert_eq!(function_groups(&report), expected);
    }
    let dropped = Echoes::default()
        .functions()
        .fn_min_tokens(6)
        .run(BOOLEAN_MATCH_FIXTURE)
        .unwrap();
    assert_eq!(function_groups(&dropped).len(), 0);
}

const CONCAT_FIXTURE: &str = "tests/fixtures/concatws";

#[test]
fn concat_doctests_group_only_the_built_in_string_literal_forms() {
    let report = Echoes::default().run(CONCAT_FIXTURE).unwrap();
    assert_eq!(report.total, 7);
    assert_eq!(report.unique, 5);
    let expected = [vec![
        "concatws::Concatenated",
        "concatws::Escaped",
        "concatws::Literal",
    ]];
    assert_eq!(doctest_groups(&report), expected);
    let kept = Echoes::default()
        .min_tokens(11)
        .run(CONCAT_FIXTURE)
        .unwrap();
    assert_eq!(doctest_groups(&kept), expected);
    let dropped = Echoes::default()
        .min_tokens(12)
        .run(CONCAT_FIXTURE)
        .unwrap();
    assert_eq!(dropped.groups.len(), 0);
}

#[test]
fn concat_functions_keep_non_string_arguments_separate() {
    let expected = [vec![
        "concatws::concatenated",
        "concatws::escaped",
        "concatws::literal",
    ]];
    for tokens in [0, 1] {
        let report = Echoes::default()
            .functions()
            .fn_min_tokens(tokens)
            .run(CONCAT_FIXTURE)
            .unwrap();
        assert_eq!(function_groups(&report), expected);
    }
    let dropped = Echoes::default()
        .functions()
        .fn_min_tokens(2)
        .run(CONCAT_FIXTURE)
        .unwrap();
    assert_eq!(function_groups(&dropped).len(), 0);
}

const BOOLEAN_LITERAL_FIXTURE: &str = "tests/fixtures/boollitws";

#[test]
fn boolean_literal_doctests_group_only_the_proven_negations() {
    let report = Echoes::default().run(BOOLEAN_LITERAL_FIXTURE).unwrap();
    assert_eq!(report.total, 6);
    assert_eq!(report.unique, 4);
    let expected = [vec![
        "boollitws::Compared",
        "boollitws::Negated",
        "boollitws::Reversed",
    ]];
    assert_eq!(doctest_groups(&report), expected);
    let kept = Echoes::default()
        .min_tokens(12)
        .run(BOOLEAN_LITERAL_FIXTURE)
        .unwrap();
    assert_eq!(doctest_groups(&kept), expected);
    let dropped = Echoes::default()
        .min_tokens(13)
        .run(BOOLEAN_LITERAL_FIXTURE)
        .unwrap();
    assert_eq!(dropped.groups.len(), 0);
}

#[test]
fn boolean_literal_functions_keep_effects_and_polarity_separate() {
    let scan = |tokens| {
        Echoes::default()
            .functions()
            .fn_min_tokens(tokens)
            .run(BOOLEAN_LITERAL_FIXTURE)
            .unwrap()
    };
    let negations = vec![
        "boollitws::compared",
        "boollitws::negated",
        "boollitws::reversed",
    ];
    let identities = vec!["boollitws::identity", "boollitws::kept"];
    for tokens in [0, 1] {
        assert_eq!(
            function_groups(&scan(tokens)),
            [negations.clone(), identities.clone()]
        );
    }
    assert_eq!(function_groups(&scan(2)), [negations]);
    assert_eq!(function_groups(&scan(3)).len(), 0);
}

const BOOLEAN_LOGIC_FIXTURE: &str = "tests/fixtures/boollogicws";

#[test]
fn boolean_logic_doctests_group_only_the_proven_conjunctions() {
    let report = Echoes::default().run(BOOLEAN_LOGIC_FIXTURE).unwrap();
    assert_eq!(report.total, 6);
    assert_eq!(report.unique, 4);
    let expected = [vec![
        "boollogicws::Commuted",
        "boollogicws::Conditional",
        "boollogicws::Padded",
    ]];
    assert_eq!(doctest_groups(&report), expected);
    let kept = Echoes::default()
        .min_tokens(18)
        .run(BOOLEAN_LOGIC_FIXTURE)
        .unwrap();
    assert_eq!(doctest_groups(&kept), expected);
    let dropped = Echoes::default()
        .min_tokens(19)
        .run(BOOLEAN_LOGIC_FIXTURE)
        .unwrap();
    assert_eq!(dropped.groups.len(), 0);
}

#[test]
fn boolean_logic_functions_keep_effects_and_operators_separate() {
    let scan = |tokens| {
        Echoes::default()
            .functions()
            .fn_min_tokens(tokens)
            .run(BOOLEAN_LOGIC_FIXTURE)
            .unwrap()
    };
    let expected = [vec![
        "boollogicws::commuted",
        "boollogicws::conditional",
        "boollogicws::padded",
    ]];
    for tokens in [0, 3] {
        assert_eq!(function_groups(&scan(tokens)), expected);
    }
    assert_eq!(function_groups(&scan(4)).len(), 0);
}

const COMPOUND_FIXTURE: &str = "tests/fixtures/compoundws";

#[test]
fn compound_doctests_group_only_the_proven_local_targets() {
    let report = Echoes::default().run(COMPOUND_FIXTURE).unwrap();
    assert_eq!(report.total, 5);
    assert_eq!(report.unique, 3);
    let expected = [vec![
        "compoundws::Compound",
        "compoundws::Expanded",
        "compoundws::Reordered",
    ]];
    assert_eq!(doctest_groups(&report), expected);
    let kept = Echoes::default()
        .min_tokens(27)
        .run(COMPOUND_FIXTURE)
        .unwrap();
    assert_eq!(doctest_groups(&kept), expected);
    let dropped = Echoes::default()
        .min_tokens(28)
        .run(COMPOUND_FIXTURE)
        .unwrap();
    assert_eq!(dropped.groups.len(), 0);
}

#[test]
fn compound_functions_keep_field_targets_separate() {
    let scan = |tokens| {
        Echoes::default()
            .functions()
            .fn_min_tokens(tokens)
            .run(COMPOUND_FIXTURE)
            .unwrap()
    };
    let expected = [vec![
        "compoundws::compound",
        "compoundws::expanded",
        "compoundws::reordered",
    ]];
    for tokens in [0, 13] {
        assert_eq!(function_groups(&scan(tokens)), expected);
    }
    assert_eq!(function_groups(&scan(14)).len(), 0);
}

const LET_LITERAL_FIXTURE: &str = "tests/fixtures/letlitws";

#[test]
fn let_literal_doctests_group_only_the_fixed_width_annotations() {
    let report = Echoes::default().run(LET_LITERAL_FIXTURE).unwrap();
    assert_eq!(report.total, 5);
    assert_eq!(report.unique, 3);
    let expected = [vec![
        "letlitws::Annotated",
        "letlitws::Hexadecimal",
        "letlitws::Suffixed",
    ]];
    assert_eq!(doctest_groups(&report), expected);
    let kept = Echoes::default()
        .min_tokens(22)
        .run(LET_LITERAL_FIXTURE)
        .unwrap();
    assert_eq!(doctest_groups(&kept), expected);
    let dropped = Echoes::default()
        .min_tokens(23)
        .run(LET_LITERAL_FIXTURE)
        .unwrap();
    assert_eq!(dropped.groups.len(), 0);
}

#[test]
fn let_literal_functions_keep_pointer_sized_annotations_separate() {
    let scan = |tokens| {
        Echoes::default()
            .functions()
            .fn_min_tokens(tokens)
            .run(LET_LITERAL_FIXTURE)
            .unwrap()
    };
    let expected = [vec![
        "letlitws::annotated",
        "letlitws::hexadecimal",
        "letlitws::suffixed",
    ]];
    for tokens in [0, 12] {
        assert_eq!(function_groups(&scan(tokens)), expected);
    }
    assert_eq!(function_groups(&scan(13)).len(), 0);
}

const CONTEXT_FIXTURE: &str = "tests/fixtures/contextws";

fn context_scan() -> Report {
    Echoes::default()
        .context_blocks()
        .context_min_tokens(0)
        .run(CONTEXT_FIXTURE)
        .unwrap()
}

fn context_items(group: &echoes::ContextGroup) -> Vec<&str> {
    group.sites.iter().map(|s| s.item.as_str()).collect()
}

#[test]
fn renamed_locals_and_captures_group_across_functions() {
    let report = context_scan();
    let group = report
        .context_groups
        .iter()
        .find(|g| g.sites.iter().any(|s| s.item.ends_with("::count_up")))
        .expect("count_up context group");
    assert_eq!(
        context_items(group),
        vec!["contextws::count_up", "contextws::count_down"]
    );
    assert!(
        group
            .sites
            .iter()
            .all(|s| s.kind == ContextKind::FunctionBody)
    );
    assert!(group.sites.iter().all(|s| s.file.ends_with("src/lib.rs")));
}

#[test]
fn closure_captures_merge_and_distinct_references_do_not() {
    let report = context_scan();
    let closures = report
        .context_groups
        .iter()
        .find(|g| {
            g.sites.iter().all(|s| s.kind == ContextKind::Closure)
                && g.sites.iter().any(|s| s.item.ends_with("::mixed"))
        })
        .expect("closure group with the mixed extra site");
    // The repeated-reference closure in mixed stays out of this group,
    // so mixed contributes exactly one site.
    assert_eq!(
        context_items(closures),
        vec!["contextws::shifted", "contextws::moved", "contextws::mixed"]
    );
    assert!(report.context_groups.iter().all(|g| {
        g.sites
            .iter()
            .filter(|s| s.item.ends_with("::mixed"))
            .count()
            <= 1
    }));
    let functions = report
        .context_groups
        .iter()
        .find(|g| {
            g.sites.iter().all(|s| s.kind == ContextKind::FunctionBody)
                && g.sites.iter().any(|s| s.item.ends_with("::shifted"))
        })
        .expect("shifted/moved function group");
    assert_eq!(
        context_items(functions),
        vec!["contextws::shifted", "contextws::moved"]
    );
}

#[test]
fn distinct_import_targets_do_not_group() {
    let report = context_scan();
    assert!(report.context_groups.iter().all(|g| {
        !(g.sites.iter().any(|s| s.item.ends_with("::via_first"))
            && g.sites.iter().any(|s| s.item.ends_with("::via_second")))
    }));
}

#[test]
fn contained_block_group_survives_with_an_extra_site() {
    let report = context_scan();
    let blocks = report
        .context_groups
        .iter()
        .find(|g| {
            g.sites.iter().all(|s| s.kind == ContextKind::Block)
                && g.sites.iter().any(|s| s.item.ends_with("::bare"))
        })
        .expect("small block group with its extra site");
    assert_eq!(
        context_items(blocks),
        vec![
            "contextws::framed_left",
            "contextws::framed_right",
            "contextws::bare"
        ]
    );
    let functions = report
        .context_groups
        .iter()
        .find(|g| {
            g.sites.iter().all(|s| s.kind == ContextKind::FunctionBody)
                && g.sites.iter().any(|s| s.item.ends_with("::framed_left"))
        })
        .expect("framed function group");
    assert_eq!(
        context_items(functions),
        vec!["contextws::framed_left", "contextws::framed_right"]
    );
}

#[test]
fn same_line_context_sites_keep_their_columns() {
    let report = context_scan();
    let group = report
        .context_groups
        .iter()
        .find(|g| g.sites.iter().any(|s| s.item.ends_with("::paired")))
        .expect("paired closure group");
    assert_eq!(group.sites.len(), 2);
    assert!(group.sites.iter().all(|s| s.kind == ContextKind::Closure));
    let (a, b) = (&group.sites[0], &group.sites[1]);
    assert_eq!(a.line, b.line);
    assert_ne!(a.column, b.column);
    assert!(a.end_column <= b.column);
}

#[test]
fn the_default_context_floor_drops_the_fixture_contexts() {
    let report = Echoes::default()
        .context_blocks()
        .run(CONTEXT_FIXTURE)
        .unwrap();
    assert!(report.context_blocks > 0);
    assert_eq!(report.unique_context_blocks, 0);
    assert_eq!(report.context_groups.len(), 0);
}

#[test]
fn exit_code_reflects_context_groups() {
    let report = context_scan();
    assert_eq!(
        echoes::exit_code(&report, false),
        std::process::ExitCode::from(1)
    );
    assert_eq!(
        echoes::exit_code(&report, true),
        std::process::ExitCode::SUCCESS
    );
}

#[test]
fn config_file_enables_context_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".echoes.toml");
    std::fs::write(&path, "context-blocks = true\ncontext-min-tokens = 0\n").unwrap();
    let report = Echoes::default()
        .config(&path)
        .unwrap()
        .run(CONTEXT_FIXTURE)
        .unwrap();
    assert!(
        report
            .context_groups
            .iter()
            .any(|g| g.sites.iter().any(|s| s.item.ends_with("::count_up")))
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".echoes.toml");
    std::fs::write(&path, "context-blocks = true\n").unwrap();
    let report = Echoes::default()
        .config(&path)
        .unwrap()
        .run(CONTEXT_FIXTURE)
        .unwrap();
    assert!(report.context_blocks > 0);
    assert_eq!(report.context_groups.len(), 0);
}

#[test]
#[cfg(feature = "cli")]
fn context_flags_run_the_scan_end_to_end() {
    let bin = env!("CARGO_BIN_EXE_cargo-echoes");
    let out = Command::new(bin)
        .current_dir(CONTEXT_FIXTURE)
        .args([
            "echoes",
            "--context-blocks",
            "--context-min-tokens",
            "0",
            "--json",
        ])
        .output()
        .expect("run echoes binary");
    assert_eq!(
        out.status.code(),
        Some(1),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json output");
    assert!(
        value["context_blocks"]
            .as_u64()
            .expect("context block count")
            > 0
    );
    let groups = value["context_groups"].as_array().expect("context groups");
    for group in groups {
        assert!(group["id"].is_string());
        assert!(group["hash"].is_string());
        assert!(group["tokens"].is_u64());
        for site in group["sites"].as_array().expect("context sites") {
            assert!(site["file"].is_string());
            assert!(site["line"].is_u64());
            assert!(site["column"].is_u64());
            assert!(site["end"].is_u64());
            assert!(site["end_column"].is_u64());
            assert!(site["item"].is_string());
            assert!(matches!(
                site["kind"].as_str(),
                Some("function-body" | "block" | "arm" | "closure")
            ));
        }
    }
    let renamed = groups
        .iter()
        .find(|group| {
            group["sites"]
                .as_array()
                .unwrap()
                .iter()
                .any(|site| site["item"] == "contextws::count_up")
        })
        .expect("renamed function group");
    let items: Vec<_> = renamed["sites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|site| site["item"].as_str().unwrap())
        .collect();
    assert_eq!(items, ["contextws::count_up", "contextws::count_down"]);
}

#[test]
#[cfg(feature = "cli")]
fn context_default_floor_and_no_fail_from_the_command_line() {
    let bin = env!("CARGO_BIN_EXE_cargo-echoes");
    let out = Command::new(bin)
        .current_dir(CONTEXT_FIXTURE)
        .args(["echoes", "--context-blocks", "--json"])
        .output()
        .expect("run echoes binary");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json output");
    assert!(
        value["context_blocks"]
            .as_u64()
            .expect("context block count")
            > 0
    );
    assert_eq!(value["context_groups"].as_array().unwrap().len(), 0);
    let out = Command::new(bin)
        .current_dir(CONTEXT_FIXTURE)
        .args([
            "echoes",
            "--context-blocks",
            "--context-min-tokens",
            "0",
            "--no-fail",
        ])
        .output()
        .expect("run echoes binary");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
#[cfg(feature = "cli")]
fn context_annotations_are_approximate_with_coordinates() {
    let bin = env!("CARGO_BIN_EXE_cargo-echoes");
    let out = Command::new(bin)
        .current_dir(CONTEXT_FIXTURE)
        .args([
            "echoes",
            "--context-blocks",
            "--context-min-tokens",
            "0",
            "--github",
        ])
        .output()
        .expect("run echoes binary");
    assert_eq!(
        out.status.code(),
        Some(1),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).expect("utf8 output");
    let marks: Vec<std::collections::BTreeMap<_, _>> = text
        .lines()
        .filter_map(|line| line.strip_prefix("::error "))
        .map(|mark| {
            mark.split_once("::")
                .unwrap()
                .0
                .split(',')
                .filter_map(|field| field.split_once('='))
                .collect()
        })
        .collect();
    let paired: Vec<_> = marks
        .iter()
        .filter(|mark| mark.get("line") == Some(&"86"))
        .map(|mark| (mark["col"], mark["endLine"], mark["endColumn"]))
        .collect();
    assert_eq!(paired, [("19", "86", "32"), ("35", "86", "48")]);
    assert!(
        marks
            .iter()
            .all(|mark| mark.get("file") == Some(&"src/lib.rs"))
    );
}
