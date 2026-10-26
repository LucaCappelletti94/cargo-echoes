//! Smoke harness for `action.yml`, ported from the Python harness.
//!
//! One test per scenario. Each runs the real `run` steps of `action.yml`
//! through a fake runner with fixture `cargo` and `gh` binaries on the
//! `PATH`, one fixture report, and a fake `github` context, and asserts on
//! the recorded calls, summaries, and status files.

#[path = "action/mod.rs"]
mod action;

use std::fs;
use std::path::Path;

use action::fix::{
    Cand, coverage, ctx_group, doc_site, group, issue_url, report, reuse, reuse_group, review,
    site, thread,
};
use action::runner::{Runner, Use};
use action::world::{SRC_DUP, SRC_LIB, SRC_PARSER, World, repo_id};
use serde_json::{Value, json};

const CACHE_KEY_PREFIX: &str = "echoes-context-index-Linux-";

fn use_by_name<'a>(uses: &'a [Use], name: &str) -> Option<&'a Use> {
    uses.iter().find(|u| u.name == name)
}

fn joined_bodies(files: &std::collections::BTreeMap<String, String>) -> String {
    files.values().cloned().collect::<Vec<_>>().join("\n")
}

#[test]
fn s01_gate_clean() {
    let mut world = World::new("s01")
        .inputs(&[("install", "true")])
        .json(report(&[], &[], None, 6, 3))
        .human("0 doctests, 0 unique, 0 functions, 0 unique functions, 0 duplicated groups\n");
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "clean gate run exits 0");
    assert!(
        r.summary().contains("### echoes"),
        "the run summary carries the echoes header"
    );
    assert!(
        r.summary().contains("duplicated groups"),
        "the run summary carries the scan line"
    );
    assert!(
        r.summary().contains("```"),
        "the gate summary fences the scan output"
    );
    let scan = r.scan_args(0);
    assert!(
        scan.iter().any(|a| a == "--github"),
        "annotations are requested in gate mode"
    );
    assert!(
        !scan.iter().any(|a| a == "--context-dependency-depth"),
        "no dependency flags at depth zero"
    );
    let install: Vec<Value> = r
        .cargo_calls()
        .into_iter()
        .filter(|c| c.get("cmd").and_then(Value::as_str) == Some("install"))
        .collect();
    assert_eq!(
        install.len(),
        1,
        "the install step used the fake cargo install"
    );
    assert_eq!(r.comment_posts().len(), 0, "no review comments are posted");
    assert_eq!(r.uses().len(), 0, "no cache step runs at depth zero");
}

#[test]
fn s02_gate_dups() {
    let mut world = World::new("s02")
        .inputs(&[("cwd", "pkg")])
        .json(report(
            &[group(
                "g1",
                "H1",
                &[
                    doc_site("src/lib.rs", 3, 5, "app::parse"),
                    doc_site("src/dup.rs", 3, 5, "app::parse"),
                ],
            )],
            &[],
            None,
            6,
            3,
        ))
        .human("2 doctests, 1 unique, 0 functions, 0 unique functions, 1 duplicated groups\n\ndoctest group g1, 2 copies\n- src/lib.rs:3  app::parse\n- src/dup.rs:3  app::parse\n\n::error file=src/lib.rs,line=3,title=echoes::app::parse duplicates the doctest at src/dup.rs:3, group 1\n")
        .exit_code(1);
    world.write_file("pkg/src/lib.rs", SRC_LIB);
    world.set_meta_root(world.workspace_path().join("pkg"));
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 1, "duplicate gate run exits 1");
    let scan_out = r
        .step_stdout("Scan for duplicates")
        .expect("the scan step ran");
    assert!(
        scan_out.contains("::error file=pkg/src/lib.rs,line=3,"),
        "annotation paths are rooted at the repository workspace"
    );
    assert!(
        r.summary().contains("duplicated groups"),
        "the gate summary keeps the scan text"
    );
    assert!(
        !r.summary().contains("::error"),
        "workflow commands stay out of the summary"
    );
}

#[test]
fn s03_gate_reuse_gated() {
    let mut world = World::new("s03")
        .inputs(&[
            ("context-dependency-depth", "2"),
            ("context-fail-on-reuse", "true"),
            ("context-select", "crate:app"),
        ])
        .json(report(
            &[],
            &[],
            Some(&reuse(
                &[reuse_group(
                    "R1",
                    &site("src/parser.rs", 3, 6, "app::parser::selected"),
                    &[
                        Cand::new("direct", "1.2.3", 1, "direct::helper", "reuse-api")
                            .repository("upstream/direct")
                            .api_path("direct::public::helper")
                            .source_url("https://github.com/upstream/direct/blob/abcdef1234567890abcdef1234567890abcdef12/src/lib.rs#L20-L24")
                            .build(),
                    ],
                )],
                Some(coverage(true, 1, 1, 0, &[], &[])),
            )),
            6,
            3,
        ))
        .human("1 doctest, 1 unique, 0 functions, 0 unique functions, 0 duplicated groups\n\ndependency reuse, 1 local context\n- src/parser.rs:3  app::parser::selected\n  - direct 1.2.3 (depth 1): the public item `direct::public::helper` can be reused\n")
        .exit_code(1);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 1, "explicit reuse gating fails the gate run");
    let args = r.scan_args(0);
    for flag in [
        "--context-fail-on-reuse",
        "--context-select",
        "crate:app",
        "--context-dependency-depth",
        "2",
        "--context-cache-dir",
    ] {
        assert!(args.iter().any(|a| a == flag), "the scan passes {flag}");
    }
    assert!(
        !args.iter().any(|a| a == "--context-check-latest"),
        "the default latest-release check is not restated"
    );
    assert_eq!(
        r.step_outputs()
            .get("scan")
            .and_then(|o| o.get("index-ok"))
            .map(String::as_str),
        Some("true"),
        "a findings gate run with complete coverage marks the index save-worthy"
    );
    let uses = r.uses();
    assert!(
        use_by_name(uses, "Restore the dependency index").is_some(),
        "the index is restored at positive depth"
    );
    assert_eq!(
        r.step_outcome("Save the dependency index"),
        "used",
        "the valid index is saved after the findings gate failure"
    );
    let cache_dir = r
        .step_outputs()
        .get("cache-dir")
        .and_then(|o| o.get("dir"))
        .map_or("", String::as_str);
    assert!(
        !cache_dir.is_empty() && Path::new(cache_dir).join("public").is_dir(),
        "the resolve step prepares the public cache subtree"
    );
}

#[test]
fn s04_pr_local_onlynew() {
    let base_report = report(
        &[group(
            "g1",
            "H1",
            &[doc_site("src/lib.rs", 3, 5, "app::parse")],
        )],
        &[],
        None,
        1,
        1,
    );
    let mut world = World::pull_request("s04")
        .git()
        .json(report(
            &[group(
                "g1",
                "H1",
                &[
                    doc_site("src/lib.rs", 3, 5, "app::parse"),
                    doc_site("src/dup.rs", 3, 5, "app::parse"),
                ],
            )],
            &[],
            None,
            6,
            3,
        ))
        .base_json(base_report)
        .changed(&["Cargo.toml", "src/lib.rs", "src/dup.rs"]);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "the only-new review run exits 0");
    assert_eq!(r.status(), "posted", "the review is posted");
    let comments = r.comment_posts();
    assert_eq!(
        comments.len(),
        1,
        "only the site introduced by the pull request is commented"
    );
    let params = comments[0].get("params").cloned().unwrap_or(Value::Null);
    assert_eq!(
        params.get("path").and_then(Value::as_str),
        Some("src/dup.rs"),
        "the comment targets the added copy"
    );
    assert_eq!(
        params.get("start_line").and_then(Value::as_str),
        Some("3"),
        "the comment starts on the added fence"
    );
    let reviews = r.review_posts();
    assert_eq!(reviews.len(), 1, "one review is posted");
    assert_eq!(
        reviews[0]
            .get("params")
            .and_then(|p| p.get("event"))
            .and_then(Value::as_str),
        Some("REQUEST_CHANGES"),
        "local duplicates keep the existing request-changes behavior"
    );
    let body = reviews[0]["params"]["body"].as_str().unwrap();
    assert!(
        body.contains("https://github.com/LucaCappelletti94/cargo-echoes)")
            && body.contains("issues/new?template=wrong-review.yml")
            && body.contains("https://github.com/sponsors/LucaCappelletti94"),
        "the review body links the project, feedback form and sponsorship"
    );
    let text = joined_bodies(&r.body_files());
    assert!(
        text.contains("added by this pull request"),
        "the comment names the added-copy case"
    );
    assert!(
        text.contains("```suggestion"),
        "the delete suggestion is kept for local duplicates"
    );
    let base_args = r.scan_args(1);
    let threshold = base_args.iter().position(|a| a == "--threshold");
    assert!(
        threshold.is_some() && base_args[threshold.unwrap() + 1] == "1",
        "the base scan inventories every site"
    );
    assert!(
        r.summary().contains("Copies to remove"),
        "the summary lists the copies to remove"
    );
}

#[test]
fn s50_exclude_reaches_the_scan_and_the_base_scan() {
    let mut world = World::pull_request("s50")
        .inputs(&[("exclude", "benches/**\n  examples/ \n\n")])
        .git()
        .json(report(&[], &[], None, 6, 3))
        .base_json(report(&[], &[], None, 1, 1));
    assert_eq!(world.run(), 0);
    let r = world.runner();
    for call in 0..2 {
        let args = r.scan_args(call);
        let excludes: Vec<&str> = args
            .windows(2)
            .filter(|pair| pair[0] == "--exclude")
            .map(|pair| pair[1].as_str())
            .collect();
        assert_eq!(excludes, ["benches/**", "examples/"], "scan {call}");
    }
}

/// The reuse report of the four candidate shapes the rendering covers.
fn s05_report() -> Value {
    let local = site("src/parser.rs", 3, 6, "app::parser::selected");
    let url_direct = "https://github.com/upstream/direct/blob/abcdef1234567890abcdef1234567890abcdef12/src/lib.rs#L20-L24";
    let candidates = vec![
        Cand::new("direct", "1.2.3", 1, "direct::helper", "reuse-api")
            .repository("upstream/direct")
            .api_path("direct::public::helper")
            .source_url(url_direct)
            .build(),
        Cand::new("transitive", "2.0.0", 2, "transitive::helper", "upgrade-api")
            .repository("upstream/transitive")
            .api_path("transitive::helper")
            .source_url("https://github.com/upstream/transitive/blob/1234567890abcdef1234567890abcdef12345678/src/lib.rs#L20-L24")
            .existing_issue("https://github.com/upstream/transitive/issues/41", "open")
            .build(),
        Cand::new(
            "direct",
            "1.2.3",
            1,
            "direct::internal_helper",
            "request-extraction",
        )
        .access("internal-function")
        .build(),
        Cand::new("direct", "1.2.3", 1, "direct::unclear", "unresolved")
            .lookup_warning("issue lookup unavailable: HTTP 503")
            .build(),
    ];
    report(
        &[],
        &[],
        Some(&reuse(
            &[reuse_group("R1", &local, &candidates)],
            Some(coverage(
                true,
                3,
                4,
                1,
                &[],
                &["cache: restored 1 file from a stale entry"],
            )),
        )),
        6,
        3,
    )
}

#[test]
fn s05_pr_reuse_only() {
    let mut world = World::pull_request("s05")
        .inputs(&[("context-dependency-depth", "2")])
        .git()
        .json(s05_report())
        .base_json(report(&[], &[], None, 6, 3))
        .changed(&["Cargo.toml", "src/lib.rs", "src/parser.rs"]);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "advisory reuse findings do not fail the run");
    assert!(
        r.review_posts().is_empty(),
        "advice-only findings need no summary review"
    );
    assert_eq!(
        r.comment_posts().len(),
        1,
        "one neutral entry is posted on the local context"
    );
    let text = joined_bodies(&r.body_files());
    assert!(
        !text.contains("```suggestion"),
        "reuse advice carries no deletion suggestion"
    );
    assert!(
        text.contains(&format!(
            "{ECHOES} found this code in a dependency. These items match it, the public ones first."
        )) && text.contains("can be reused"),
        "the reuse advice names echoes and renders the accessible API alternative"
    );
    assert!(
        text.contains("direct::public::helper"),
        "the advice names the callable public route"
    );
    assert!(
        text.contains("source (`direct::helper`)"),
        "the source evidence keeps the original site item"
    );
    assert!(
        text.contains("https://github.com/upstream/transitive/issues/41"),
        "the existing upstream request is linked"
    );
    assert!(
        text.contains("existing request open"),
        "the existing request state is shown"
    );
    assert!(
        text.contains("the upstream repository is not declared"),
        "the unknown repository is rendered explicitly"
    );
    assert!(
        text.contains("lookup: `issue lookup unavailable: HTTP 503`"),
        "the failed issue lookup is rendered explicitly"
    );
    assert!(
        text.contains("Report a wrong review")
            && text.contains("reuse%20group%20R1")
            && text.contains("src%2Fparser.rs%3A3%20%20app%3A%3Aparser%3A%3Aselected")
            && text.contains("direct%201.2.3%20%20direct%3A%3Ahelper"),
        "the reuse advice links a wrong-review issue naming the local site and the candidates"
    );
    s05_summary_checks(&r.summary());
}

/// The run summary of the reuse report, its section, candidate order and coverage.
fn s05_summary_checks(summary: &str) {
    assert!(
        summary.contains("### Dependency reuse"),
        "the run summary reports the reuse groups"
    );
    let order = [
        summary.find("can be reused"),
        summary.find("newer published release"),
        summary.find("no issue link is offered"),
        summary.find("does not establish accessibility"),
    ];
    let sorted = {
        let mut s = order;
        s.sort();
        s
    };
    assert!(
        order.iter().all(Option::is_some) && order == sorted,
        "the summary keeps the report candidate order, accessible APIs first"
    );
    assert!(
        summary.contains("3 packages, 4 files indexed, 1 from cache"),
        "the summary reports the dependency coverage"
    );
    assert!(
        summary.contains("cache: restored 1 file from a stale entry"),
        "the coverage warnings are rendered"
    );
    assert!(
        !summary.contains("incomplete"),
        "a complete analysis is not flagged incomplete"
    );
}

#[test]
fn s06_pr_context_groups() {
    let mut world = World::pull_request("s06")
        .inputs(&[("context-select", "crate:app")])
        .git()
        .json(report(
            &[],
            &[ctx_group(
                "c9f8",
                "C1",
                &[
                    site("src/parser.rs", 3, 6, "app::parser::selected"),
                    site("src/mod.rs", 10, 13, "app::other::selected"),
                ],
            )],
            None,
            6,
            3,
        ))
        .base_json(report(&[], &[], None, 6, 3))
        .changed(&["Cargo.toml", "src/lib.rs", "src/parser.rs", "src/mod.rs"])
        .extra_base("src/mod.rs", SRC_PARSER);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "the context group run exits 0");
    assert!(
        r.review_posts().is_empty(),
        "context advice needs no summary review"
    );
    assert_eq!(
        r.comment_posts().len(),
        1,
        "the kept context copy stays silent"
    );
    let text = joined_bodies(&r.body_files());
    assert!(
        text.contains(&format!(
            "{ECHOES} found this code at the places below too. A shared helper could hold it once."
        )) && text
            .contains("Check that the copies agree in types and behaviour before sharing them."),
        "the context advice names echoes and reads plainly"
    );
    assert!(
        !text.contains("```suggestion"),
        "approximate contexts get no deletion suggestion"
    );
    assert!(
        !text.contains("remove") && !text.contains("delet"),
        "the context advice never suggests deletion"
    );
    assert!(
        text.contains("Report a wrong review")
            && text.contains("context%20group%20C1")
            && text.contains("src%2Fparser.rs%3A3%20%20app%3A%3Aparser%3A%3Aselected")
            && text.contains("src%2Fmod.rs%3A10%20%20app%3A%3Aother%3A%3Aselected"),
        "the context advice links a wrong-review issue naming every copy"
    );
    assert!(
        text.contains("  - [`src/parser.rs:3-6`](")
            && text.contains("/src/parser.rs#L3-L6)  `app::parser::selected`"),
        "the other context copy links its lines at the head commit"
    );
    assert!(
        text.contains(&thread_marker(
            ".",
            "context%3AC1%3Asrc%2Fmod.rs%3Aapp%3A%3Aother%3A%3Aselected"
        )),
        "the comment carries its workspace and finding key"
    );
    let summary = r.summary();
    assert!(
        summary.contains("### Context groups"),
        "the run summary reports the context groups"
    );
    assert!(
        summary.contains("c9f8")
            && summary.contains("src/parser.rs:3")
            && summary.contains("src/mod.rs:10"),
        "the summary keeps the complete selected-corpus context group"
    );
}

#[test]
fn s07_pr_fork_403() {
    let mut world = World::pull_request("s07")
        .inputs(&[("only-new", "false")])
        .fork()
        .json(report(
            &[group(
                "g1",
                "H1",
                &[
                    doc_site("src/lib.rs", 3, 5, "app::parse"),
                    doc_site("src/dup.rs", 3, 5, "app::parse"),
                ],
            )],
            &[],
            None,
            6,
            3,
        ))
        .changed(&["src/lib.rs", "src/dup.rs"])
        .gh_403();
    world.write_file("src/lib.rs", SRC_LIB);
    world.write_file("src/dup.rs", SRC_DUP);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "a fork pull request does not fail the job");
    assert_eq!(
        r.status(),
        "skipped-readonly",
        "the review is skipped as read-only"
    );
    assert_eq!(
        r.comment_posts().len(),
        1,
        "the permission probe is attempted"
    );
    assert_eq!(
        r.review_posts().len(),
        0,
        "no review is posted without permission"
    );
    assert!(
        r.summary().contains("### echoes"),
        "the public report stays available on forks"
    );
}

#[test]
fn s08_pr_incomplete() {
    let local = site("src/parser.rs", 3, 6, "app::parser::selected");
    let mut world = World::pull_request("s08").inputs(&[("context-dependency-depth", "2")])
        .json(report(
            &[],
            &[],
            Some(&reuse(
                &[reuse_group(
                    "R1",
                    &local,
                    &[
                        Cand::new("direct", "1.2.3", 1, "direct::helper", "reuse-api")
                            .repository("upstream/direct")
                            .source_url("https://github.com/upstream/direct/blob/abcdef1234567890abcdef1234567890abcdef12/src/lib.rs#L20-L24")
                            .build(),
                    ],
                )],
                Some(coverage(
                    false,
                    1,
                    1,
                    0,
                    &["source indexing failed: direct@1.2.3 (parse error)"],
                    &[],
                )),
            )),
            6,
            3,
        ))
        .changed(&["src/parser.rs"])
        .exit_code(2);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 2, "an incomplete dependency analysis fails the run");
    let summary = r.summary();
    assert!(
        summary.contains("### Dependency reuse"),
        "the available findings are rendered before the failure"
    );
    assert!(
        summary.contains("direct@1.2.3") && summary.contains("source indexing failed"),
        "the coverage failure is rendered"
    );
    assert!(
        summary.contains("incomplete"),
        "the incomplete analysis is flagged"
    );
    assert!(
        matches!(
            r.step_outcome("Review the findings on the pull request"),
            "skipped" | "skipped-after-failure"
        ),
        "the failed scan prevents the review from posting"
    );
    assert_eq!(
        r.review_posts().len(),
        0,
        "no review is posted on incomplete analysis"
    );
}

/// The prepare fixture report and the base inventory of the two-job flow.
fn s09_fixture() -> (Value, Value) {
    let local = site("src/parser.rs", 3, 6, "app::parser::selected");
    let url_direct = "https://github.com/upstream/direct/blob/abcdef1234567890abcdef1234567890abcdef12/src/lib.rs#L20-L24";
    let rep = report(
        &[group(
            "g1",
            "H1",
            &[
                doc_site("src/lib.rs", 3, 5, "app::parse"),
                doc_site("src/dup.rs", 3, 5, "app::parse"),
            ],
        )],
        &[],
        Some(&reuse(
            &[reuse_group(
                "R1",
                &local,
                &[
                    Cand::new("direct", "1.2.3", 1, "direct::helper", "reuse-api")
                        .repository("upstream/direct")
                        .source_url(url_direct)
                        .issue_url(&issue_url("upstream", "direct", "direct::helper"))
                        .build(),
                ],
            )],
            Some(coverage(true, 1, 1, 0, &[], &[])),
        )),
        6,
        3,
    );
    let base_report = report(
        &[group(
            "g1",
            "H1",
            &[doc_site("src/lib.rs", 3, 5, "app::parse")],
        )],
        &[],
        None,
        1,
        1,
    );
    (rep, base_report)
}

/// The prepare payload carries data for the posting job and no rendered text.
fn s09_payload_checks(world: &World, r: &Runner) {
    let payload = world.workspace_path().join("echoes-payload");
    let meta: Value = serde_json::from_str(
        &fs::read_to_string(payload.join("meta.json")).expect("read the payload metadata"),
    )
    .expect("parse the payload metadata");
    assert_eq!(meta["pr"], "7", "the payload names the pull request");
    assert_eq!(
        meta["head_sha"].as_str().map(str::len),
        Some(40),
        "the payload names the head commit"
    );
    let report: Value = serde_json::from_str(
        &fs::read_to_string(payload.join("report.json")).expect("read the payload report"),
    )
    .expect("parse the payload report");
    assert_eq!(report["groups"].as_array().map(Vec::len), Some(1));
    assert!(
        payload.join("base-report.json").exists(),
        "only-new needs the base report"
    );
    for entry in fs::read_dir(&payload).expect("list the payload") {
        let text = fs::read_to_string(entry.expect("payload entry").path()).unwrap_or_default();
        assert!(
            !text.contains("Report a wrong review"),
            "the payload carries no rendered comment"
        );
    }
    assert_eq!(r.comment_posts().len(), 0, "the prepare run posts nothing");
}

#[test]
fn s09_prepare_post_reuse() {
    let (rep, base_report) = s09_fixture();
    let mut world = World::pull_request("s09a")
        .inputs(&[
            ("mode", "prepare"),
            ("payload", "echoes-payload"),
            ("context-dependency-depth", "2"),
        ])
        .git()
        .json(rep)
        .base_json(base_report)
        .changed(&["Cargo.toml", "src/lib.rs", "src/dup.rs", "src/parser.rs"]);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "the prepare run exits 0");
    assert_eq!(r.status(), "prepared", "the prepare run writes the status");
    s09_payload_checks(&world, r);
    let payload_ws = world.workspace_path().to_path_buf();
    let (base, head) = world.shas();
    // The posting job runs on a fresh runner sharing the prepare workspace, so
    // the payload directory outlives the prepare job.
    let mut world2 = World::pull_request("s09b")
        .inputs(&[("mode", "post"), ("payload", "echoes-payload")])
        .workspace(payload_ws)
        .base_sha(&base)
        .head_sha(&head)
        .changed(&["Cargo.toml", "src/lib.rs", "src/dup.rs", "src/parser.rs"]);
    let exit2 = world2.run();
    let r2 = world2.runner();
    assert_eq!(exit2, 0, "the post run exits 0");
    assert_eq!(
        r2.status(),
        "posted",
        "the post run posts the prepared review"
    );
    assert_eq!(r2.scan_calls().len(), 0, "the post job rescans nothing");
    assert_eq!(
        r2.comment_posts().len(),
        2,
        "the post job posts the local comment and the reuse entry"
    );
    let reviews = r2.review_posts();
    assert_eq!(
        reviews
            .first()
            .and_then(|c| c.get("params"))
            .and_then(|p| p.get("event"))
            .and_then(Value::as_str),
        Some("REQUEST_CHANGES"),
        "the post job computes the review event"
    );
}

#[test]
fn s10_dry_run_reuse() {
    let local = site("src/parser.rs", 3, 6, "app::parser::selected");
    let mut world = World::new("s10")
        .inputs(&[("dry-run", "true"), ("context-dependency-depth", "2")])
        .json(report(
            &[group(
                "g1",
                "H1",
                &[
                    doc_site("src/lib.rs", 3, 5, "app::parse"),
                    doc_site("src/dup.rs", 3, 5, "app::parse"),
                ],
            )],
            &[],
            Some(&reuse(
                &[reuse_group(
                    "R1",
                    &local,
                    &[
                        Cand::new("direct", "1.2.3", 1, "direct::helper", "reuse-api")
                            .repository("upstream/direct")
                            .source_url("https://github.com/upstream/direct/blob/abcdef1234567890abcdef1234567890abcdef12/src/lib.rs#L20-L24")
                            .build(),
                    ],
                )],
                Some(coverage(true, 1, 1, 0, &[], &[])),
            )),
            6,
            3,
        ));
    world.write_file("src/lib.rs", SRC_LIB);
    world.write_file("src/dup.rs", SRC_DUP);
    world.write_file("src/parser.rs", SRC_PARSER);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "the dry run exits 0");
    assert_eq!(r.status(), "dry-run", "the dry run writes its status");
    assert!(
        r.gh_calls()
            .iter()
            .all(|c| c.get("m").and_then(Value::as_str) != Some("POST")),
        "the dry run posts nothing to the API"
    );
    let body = r.body();
    assert!(
        body.contains("dry run, the review is not posted"),
        "the dry run says it posts nothing"
    );
    assert!(
        body.contains("Inline comments echoes would post"),
        "the dry run previews the local comments"
    );
    assert!(
        body.contains("Neutral reuse and context advice echoes would post"),
        "the dry run previews the neutral reuse advice"
    );
    assert!(
        body.contains("can be reused"),
        "the dry run preview names the accessible API"
    );
    assert!(
        r.summary().contains("### Dependency reuse"),
        "the summary reports the reuse groups"
    );
}

#[test]
fn s11_post_missing_payload() {
    let mut world = World::new("s11")
        .inputs(&[
            ("mode", "post"),
            ("payload", "empty-dir"),
            ("pr-number", "7"),
        ])
        .event("pull_request");
    fs::create_dir_all(world.workspace_path().join("empty-dir")).expect("make the payload dir");
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 1, "a missing prepared payload fails the post job");
    assert_eq!(r.status(), "rejected", "the post job records the rejection");
}

#[test]
fn s12_cache_trusted_save() {
    let mut world = World::new("s12")
        .inputs(&[("context-dependency-depth", "2")])
        .json(report(
            &[],
            &[],
            Some(&reuse(&[], Some(coverage(true, 1, 1, 0, &[], &[])))),
            6,
            3,
        ))
        .human("0 doctests, 0 unique, 0 functions, 0 unique functions, 0 duplicated groups\n");
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "the trusted run exits 0");
    let uses = r.uses();
    let restore = use_by_name(uses, "Restore the dependency index");
    let save = use_by_name(uses, "Save the dependency index");
    assert!(restore.is_some(), "the trusted run restores the index");
    assert!(
        save.is_some(),
        "the trusted successful indexing saves the index"
    );
    let cache_dir = r
        .step_outputs()
        .get("cache-dir")
        .and_then(|o| o.get("dir"))
        .map_or("", String::as_str);
    assert!(
        !cache_dir.is_empty() && Path::new(cache_dir).join("public").is_dir(),
        "the public subtree is prepared"
    );
    if let (Some(restore), Some(save)) = (restore, save) {
        let public = format!("{cache_dir}/public");
        assert_eq!(
            restore.with.get("path").map(String::as_str),
            Some(public.as_str()),
            "the cache targets the public subtree only"
        );
        assert_eq!(
            save.with.get("path").map(String::as_str),
            Some(public.as_str()),
            "the save targets the public subtree only"
        );
        let restore_key = format!("{CACHE_KEY_PREFIX}any-v1");
        assert_eq!(
            restore.with.get("key").map(String::as_str),
            Some(restore_key.as_str()),
            "the restore key names the platform and target"
        );
        assert!(
            save.with
                .get("key")
                .map(String::as_str)
                .is_some_and(|k| k.starts_with(restore_key.as_str())),
            "the save key extends the restore key with a per-run generation"
        );
        assert_ne!(
            save.with.get("key"),
            restore.with.get("key"),
            "the save key is unique so a new index generation is stored"
        );
        assert!(
            save.with
                .get("key")
                .map(String::as_str)
                .is_some_and(|k| k.starts_with(CACHE_KEY_PREFIX)),
            "the save key stays prefix-compatible with the restore fallback"
        );
        assert!(
            restore
                .with
                .get("restore-keys")
                .map(String::as_str)
                .is_some_and(|k| k.starts_with(CACHE_KEY_PREFIX)),
            "the restore falls back to the platform prefix"
        );
    }
    let args = r.scan_args(0);
    let pos = args.iter().position(|a| a == "--context-cache-dir");
    assert!(
        pos.is_some() && args[pos.unwrap() + 1] == cache_dir,
        "the scan uses the shared cache directory"
    );
}

#[test]
fn s13a_cache_fork_no_save() {
    let mut world = World::new("s13a")
        .inputs(&[("context-dependency-depth", "2")])
        .event("pull_request")
        .fork()
        .json(report(
            &[],
            &[],
            Some(&reuse(&[], Some(coverage(true, 0, 0, 0, &[], &[])))),
            6,
            3,
        ))
        .human("0 doctests, 0 unique, 0 functions, 0 unique functions, 0 duplicated groups\n");
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "the fork run exits 0");
    let uses = r.uses();
    assert!(
        use_by_name(uses, "Restore the dependency index").is_some(),
        "a fork run may restore the index"
    );
    assert!(
        use_by_name(uses, "Save the dependency index").is_none(),
        "a fork run never uploads the index"
    );
}

#[test]
fn s13b_cache_depth_zero() {
    let mut world = World::new("s13b")
        .json(report(&[], &[], None, 6, 3))
        .human("0 doctests, 0 unique, 0 functions, 0 unique functions, 0 duplicated groups\n");
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "the depth-zero run exits 0");
    assert_eq!(r.uses().len(), 0, "depth zero touches no cache step");
    let args = r.scan_args(0);
    assert!(
        !args.iter().any(|a| a == "--context-dependency-depth")
            && !args.iter().any(|a| a == "--context-cache-dir"),
        "depth zero passes no dependency flags"
    );
}

#[test]
fn s14_escape_untrusted() {
    let local = site("src/parser.rs", 3, 6, "app::parser::selected");
    let evil = r#"weird:item, 50% $(touch "$RUNNER_TEMP/pwned") `backtick` "quoted""#;
    let mut world = World::pull_request("s14").inputs(&[("context-dependency-depth", "2")])
        .git()
        .json(report(
            &[],
            &[],
            Some(&reuse(
                &[reuse_group(
                    "R1",
                    &local,
                    &[
                        Cand::new("evil-pkg", "1.2.3", 1, evil, "reuse-api")
                            .repository("upstream/evil-pkg")
                            .source_url("https://github.com/upstream/evil-pkg/blob/abcdef1234567890abcdef1234567890abcdef12/src/lib.rs#L20-L24")
                            .build(),
                    ],
                )],
                Some(coverage(true, 1, 1, 0, &[], &[])),
            )),
            6,
            3,
        ))
        .changed(&["src/parser.rs"]);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "the untrusted-string run exits 0");
    let summary = r.summary();
    assert!(
        summary.contains("$(touch"),
        "untrusted strings render literally in the summary"
    );
    assert!(
        summary.contains("`backtick`"),
        "backticks render literally in the summary"
    );
    let text = joined_bodies(&r.body_files());
    assert!(
        text.contains("$(touch"),
        "untrusted strings render literally in the review body"
    );
    assert!(
        !Path::new(r.run_temp()).join("pwned").exists(),
        "untrusted strings are never evaluated"
    );
}

#[test]
fn s15_prepare_post_reuse_only() {
    let local = site("src/parser.rs", 3, 6, "app::parser::selected");
    let mut world = World::pull_request("s15a").inputs(&[("mode", "prepare"),
    ("payload", "echoes-payload"),
    ("context-dependency-depth", "2"),])
        .git()
        .json(report(
            &[],
            &[],
            Some(&reuse(
                &[reuse_group(
                    "R1",
                    &local,
                    &[
                        Cand::new("direct", "1.2.3", 1, "direct::helper", "reuse-api")
                            .repository("upstream/direct")
                            .source_url("https://github.com/upstream/direct/blob/abcdef1234567890abcdef1234567890abcdef12/src/lib.rs#L20-L24")
                            .issue_url(&issue_url("upstream", "direct", "direct::helper"))
                            .build(),
                    ],
                )],
                Some(coverage(true, 1, 1, 0, &[], &[])),
            )),
            6,
            3,
        ))
        .base_json(report(&[], &[], None, 6, 3))
        .changed(&["Cargo.toml", "src/lib.rs", "src/parser.rs"]);
    let exit = world.run();

    assert_eq!(exit, 0, "the reuse-only prepare run exits 0");
    // The posting job shares the prepare workspace so the payload survives it.
    let payload_ws = world.workspace_path().to_path_buf();
    let (base, head) = world.shas();
    let mut world2 = World::pull_request("s15b")
        .inputs(&[("mode", "post"), ("payload", "echoes-payload")])
        .workspace(payload_ws)
        .base_sha(&base)
        .head_sha(&head)
        .changed(&["Cargo.toml", "src/lib.rs", "src/parser.rs"]);
    let exit2 = world2.run();
    let r2 = world2.runner();
    assert_eq!(exit2, 0, "the reuse-only post run exits 0");
    assert!(
        r2.review_posts().is_empty() && r2.comment_posts().len() == 1,
        "dependency-only advice posts its inline entry and no summary review through post"
    );
    assert_eq!(r2.status(), "posted", "the inline advice counts as posted");
}

#[test]
fn s16_comment_cap() {
    let mut world = World::pull_request("s16")
        .inputs(&[("only-new", "false"), ("max-comments", "2")])
        .json(report(
            &[group(
                "g1",
                "H1",
                &[
                    doc_site("src/a.rs", 3, 5, "app::a"),
                    doc_site("src/b.rs", 3, 5, "app::b"),
                    doc_site("src/c.rs", 3, 5, "app::c"),
                    doc_site("src/d.rs", 3, 5, "app::d"),
                ],
            )],
            &[],
            None,
            6,
            3,
        ))
        .changed(&["src/a.rs", "src/b.rs", "src/c.rs", "src/d.rs"]);
    world.write_file("src/a.rs", SRC_LIB);
    world.write_file("src/b.rs", SRC_LIB);
    world.write_file("src/c.rs", SRC_LIB);
    world.write_file("src/d.rs", SRC_LIB);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "the capped review run exits 0");
    assert_eq!(
        r.status(),
        "posted",
        "the review is posted under the comment cap"
    );
    assert_eq!(
        r.comment_posts().len(),
        2,
        "exactly the capped number of inline comments is posted"
    );
    let reviews = r.review_posts();
    assert_eq!(reviews.len(), 1, "one review is posted");
    let body = reviews[0]
        .get("params")
        .and_then(|p| p.get("body"))
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        body.contains("1 inline copies were not posted") && body.contains("/actions/runs/1"),
        "the review body reports the omitted copy and links the run summary"
    );
    let summary = r.summary();
    assert!(
        summary.contains("Inline copies not posted") && summary.contains("src/d.rs:3"),
        "the run summary lists the omitted copy"
    );
}

#[test]
fn s17_rate_limit_403() {
    let mut world = World::pull_request("s17")
        .inputs(&[("only-new", "false")])
        .json(report(
            &[group(
                "g1",
                "H1",
                &[
                    doc_site("src/lib.rs", 3, 5, "app::parse"),
                    doc_site("src/dup.rs", 3, 5, "app::parse"),
                    doc_site("src/dup2.rs", 3, 5, "app::parse"),
                ],
            )],
            &[],
            None,
            6,
            3,
        ))
        .changed(&["src/lib.rs", "src/dup.rs", "src/dup2.rs"])
        .rate_limit_after(1);
    world.write_file("src/lib.rs", SRC_LIB);
    world.write_file("src/dup.rs", SRC_LIB);
    world.write_file("src/dup2.rs", SRC_LIB);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "a rate-limited review run exits 0");
    assert_eq!(
        r.status(),
        "posted",
        "a secondary rate limit is not misreported as read-only"
    );
    let reviews = r.review_posts();
    assert_eq!(
        reviews.len(),
        1,
        "the review is posted despite the rate limit"
    );
    let body = reviews[0]
        .get("params")
        .and_then(|p| p.get("body"))
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        body.contains("1 inline copies were not posted because the api rate limit was reached")
            && body.contains("/actions/runs/1"),
        "the review body reports the rate-limited omission and links the run summary"
    );
    let summary = r.summary();
    assert!(
        summary.contains("Rate limit"),
        "the run summary reports the rate limit"
    );
    assert!(
        summary.contains("`src/dup2.rs:3`  `app::parse` (group 1)"),
        "the run summary lists the rate-limited copy"
    );
}

#[test]
fn s18_perm_403_readonly() {
    let mut world = World::pull_request("s18")
        .inputs(&[("only-new", "false")])
        .json(report(
            &[group(
                "g1",
                "H1",
                &[
                    doc_site("src/lib.rs", 3, 5, "app::parse"),
                    doc_site("src/dup.rs", 3, 5, "app::parse"),
                ],
            )],
            &[],
            None,
            6,
            3,
        ))
        .changed(&["src/lib.rs", "src/dup.rs"])
        .gh_403();
    world.write_file("src/lib.rs", SRC_LIB);
    world.write_file("src/dup.rs", SRC_LIB);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "a permission 403 does not fail the job");
    assert_eq!(
        r.status(),
        "skipped-readonly",
        "a permission 403 without a rate limit message is read-only"
    );
    assert_eq!(
        r.comment_posts().len(),
        1,
        "the permission probe is attempted once"
    );
    assert_eq!(
        r.review_posts().len(),
        0,
        "no review is posted without permission"
    );
}

#[test]
fn review_fallback_links_feedback_without_phantom_overflow() {
    for code in ["422", "500"] {
        let mut world = World::pull_request(&format!("review-fallback-{code}"))
            .inputs(&[("only-new", "false")])
            .json(report(
                &[],
                &[ctx_group(
                    "c1",
                    "C1",
                    &[
                        site("src/lib.rs", 3, 6, "app::original"),
                        site("src/parser.rs", 10, 14, "app::copy"),
                    ],
                )],
                None,
                6,
                3,
            ))
            .changed(&["src/parser.rs"])
            .env("SCEN_GH_FAIL", code);
        assert_eq!(world.run(), 0);
        let reviews = world.runner().review_posts();
        let body = reviews[0]["params"]["body"].as_str().unwrap();
        assert!(body.contains("src/parser.rs:10-14"));
        assert!(
            !body.contains("/actions/runs/"),
            "a complete fallback list needs no overflow link"
        );
        assert!(
            body.contains("issues/new?template=wrong-review.yml")
                && body.contains("review=https%3A%2F%2Fgithub.com%2Fowner%2Frepo%2Fpull%2F7")
                && body.contains("src%2Fparser.rs%3A10%20%20app%3A%3Acopy")
                && body.contains("https://github.com/sponsors/LucaCappelletti94")
                && body.contains("[echoes](https://github.com/LucaCappelletti94/cargo-echoes)"),
            "fallback feedback identifies the pull request and reported sites"
        );
    }
}

#[test]
fn s19_oor_truncation() {
    let mut sites = Vec::new();
    for i in 0..800 {
        sites.push(json!({
            "file": "src/lib.rs",
            "line": 10 + i * 8,
            "item": format!("app::f{i}"),
        }));
    }
    let mut world = World::pull_request("s19")
        .inputs(&[("only-new", "false")])
        .json(report(&[group("g1", "H1", &sites)], &[], None, 6, 3))
        .changed(&["src/lib.rs"]);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "the oversized review run exits 0");
    assert_eq!(
        r.status(),
        "posted",
        "the oversized review is posted truncated"
    );
    assert_eq!(
        r.comment_posts().len(),
        0,
        "out-of-range sites get no inline comment"
    );
    let reviews = r.review_posts();
    assert_eq!(reviews.len(), 1, "one review is posted");
    let body = reviews[0]
        .get("params")
        .and_then(|p| p.get("body"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let len = body.chars().count();
    assert!(
        len < 65536,
        "the review body stays under the api limit, it is {len} characters"
    );
    assert!(
        body.contains("more, they are in the run summary") && body.contains("src/lib.rs"),
        "the truncated list points at the run summary"
    );
    let listed = body
        .lines()
        .filter(|line| line.starts_with("- group "))
        .count();
    assert!(
        body.contains(&format!("and {} more,", sites.len() - 1 - listed)),
        "the overflow count includes only sites absent from the review"
    );
}

#[test]
fn s20_prepare_post_cap() {
    let local = site("src/parser.rs", 3, 6, "app::parser::selected");
    let mut world = World::pull_request("s20a")
        .inputs(&[
            ("mode", "prepare"),
            ("payload", "echoes-payload"),
            ("max-comments", "1"),
        ])
        .json(report(
            &[group(
                "g1",
                "H1",
                &[
                    doc_site("src/lib.rs", 3, 5, "app::parse"),
                    doc_site("src/dup.rs", 3, 5, "app::parse"),
                ],
            )],
            &[],
            Some(&reuse(
                &[reuse_group(
                    "R1",
                    &local,
                    &[
                        Cand::new("direct", "1.2.3", 1, "direct::helper", "reuse-api")
                            .repository("upstream/direct")
                            .build(),
                    ],
                )],
                Some(coverage(true, 1, 1, 0, &[], &[])),
            )),
            6,
            3,
        ))
        .changed(&["src/lib.rs", "src/dup.rs", "src/parser.rs"]);
    world.write_file("src/lib.rs", SRC_LIB);
    world.write_file("src/dup.rs", SRC_LIB);
    world.write_file("src/parser.rs", SRC_LIB);
    let exit = world.run();
    assert_eq!(exit, 0, "the capped prepare run exits 0");
    let r = world.runner();
    assert_eq!(r.status(), "prepared", "the prepare run writes the status");
    assert_eq!(r.comment_posts().len(), 0, "the prepare run posts nothing");
    let summary = r.summary();
    assert!(
        summary.contains("Inline copies not posted"),
        "the prepare summary lists the omitted copy"
    );
    let payload_ws = world.workspace_path().to_path_buf();
    let (base, head) = world.shas();
    let mut world2 = World::pull_request("s20b")
        .inputs(&[
            ("mode", "post"),
            ("payload", "echoes-payload"),
            ("max-comments", "1"),
        ])
        .workspace(payload_ws)
        .base_sha(&base)
        .head_sha(&head)
        .changed(&["src/lib.rs", "src/dup.rs", "src/parser.rs"]);
    let exit2 = world2.run();
    let r2 = world2.runner();
    assert_eq!(exit2, 0, "the post run exits 0");
    assert_eq!(
        r2.status(),
        "posted",
        "the post run posts the capped review"
    );
    assert_eq!(
        r2.comment_posts().len(),
        1,
        "the post job posts only the capped comments"
    );
    let reviews = r2.review_posts();
    let body = reviews[0]
        .get("params")
        .and_then(|p| p.get("body"))
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        body.contains("1 inline copies were not posted"),
        "the post job applies its own cap"
    );
    assert!(
        body.contains("https://github.com/owner/repo/actions/runs/1"),
        "the post body links the run that holds the omitted list"
    );
    let summary2 = r2.summary();
    assert!(
        summary2.contains("Inline copies not posted")
            && summary2.contains("`src/parser.rs:3`  `app::parser::selected` (reuse)"),
        "the post summary lists the omitted copy"
    );
}

/// A prepared payload for pull request 7 with one group of `copies` sites in the
/// diff and the item of every site set to `item`. `kind` is `doctest`, or the
/// remedy of a function group.
fn prepared(name: &str, copies: usize, item: &str, kind: &str) -> World {
    let files: Vec<String> = (0..copies).map(|i| format!("src/c{i}.rs")).collect();
    let sites: Vec<Value> = files
        .iter()
        .map(|f| {
            if kind == "doctest" {
                doc_site(f, 3, 5, item)
            } else {
                site(f, 3, 5, item)
            }
        })
        .collect();
    let mut g = group("g1", "H1", &sites);
    if kind != "doctest" {
        g["kind"] = json!("function");
        g["remedy"] = json!(kind);
    }
    let changed: Vec<&str> = files.iter().map(String::as_str).collect();
    let mut world = World::pull_request(name)
        .inputs(&[("mode", "prepare"), ("payload", "echoes-payload")])
        .json(report(&[g], &[], None, 6, 3))
        .changed(&changed);
    for f in &files {
        world.write_file(f, SRC_LIB);
    }
    assert_eq!(world.run(), 0, "the prepare run exits 0");
    assert_eq!(world.runner().status(), "prepared");
    world
}

/// The post world for a prepared payload, triggered by `event`.
fn post_world(name: &str, prepared: &World, event: &str, inputs: &[(&str, &str)]) -> World {
    let (base, head) = prepared.shas();
    let mut all = vec![("mode", "post"), ("payload", "echoes-payload")];
    all.extend_from_slice(inputs);
    let changed: Vec<String> = (0..8).map(|i| format!("src/c{i}.rs")).collect();
    let changed: Vec<&str> = changed.iter().map(String::as_str).collect();
    World::new(name)
        .inputs(&all)
        .event(event)
        .workspace(prepared.workspace_path().to_path_buf())
        .base_sha(&base)
        .head_sha(&head)
        .changed(&changed)
}

fn copy_payload(prepared: &World, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    let source = prepared.workspace_path().join("echoes-payload");
    for file in ["meta.json", "report.json", "base-report.json", "blank.txt"] {
        if source.join(file).is_file() {
            fs::copy(source.join(file), destination.join(file)).unwrap();
        }
    }
}

#[test]
fn post_collection_posts_seven_workspaces_with_independent_caps() {
    let prep = prepared("collection-seven", 4, "app::parse", "doctest");
    let root = prep.workspace_path().join("payloads");
    let mut changed = Vec::new();
    for workspace in [
        "connetto-web",
        "dioxus-demo",
        "root",
        "tauri-leptos-demo",
        "wasm-smoke",
        "webauthn-unlock",
        "yew-web-demo",
    ] {
        let child = root.join(format!("echoes-payload-{workspace}"));
        copy_payload(&prep, &child);
        let mut meta: Value =
            serde_json::from_str(&fs::read_to_string(child.join("meta.json")).unwrap()).unwrap();
        meta["scope"] = json!(workspace);
        fs::write(child.join("meta.json"), meta.to_string()).unwrap();
        let mut report: Value =
            serde_json::from_str(&fs::read_to_string(child.join("report.json")).unwrap()).unwrap();
        for site in report["groups"][0]["sites"].as_array_mut().unwrap() {
            let file = format!("{workspace}/{}", site["file"].as_str().unwrap());
            changed.push(file.clone());
            site["file"] = json!(file);
        }
        fs::write(child.join("report.json"), report.to_string()).unwrap();
    }
    let changed: Vec<&str> = changed.iter().map(String::as_str).collect();
    let mut post = post_world(
        "collection-seven-post",
        &prep,
        "workflow_run",
        &[("payload", "payloads"), ("max-comments", "1")],
    )
    .changed(&changed);
    assert_eq!(post.run(), 0);
    let comments = post.runner().comment_posts();
    let reviews = post.runner().review_posts();
    assert_eq!(comments.len(), 7);
    assert_eq!(reviews.len(), 7);
    let posted_paths: std::collections::BTreeSet<&str> = comments
        .iter()
        .map(|comment| comment["params"]["path"].as_str().unwrap())
        .collect();
    let expected_paths = changed
        .iter()
        .copied()
        .filter(|path| path.ends_with("/src/c1.rs"))
        .collect();
    assert_eq!(posted_paths, expected_paths);
    for review in reviews {
        assert_eq!(review["params"]["event"], "REQUEST_CHANGES");
        assert!(
            review["params"]["body"]
                .as_str()
                .unwrap()
                .contains("2 inline copies were not posted")
        );
    }
}

#[test]
fn post_collection_rejects_bad_siblings_and_posts_the_others() {
    let prep = prepared("collection-validation", 2, "app::parse", "doctest");
    let root = prep.workspace_path().join("payloads");
    for name in [
        "00-valid",
        "01-pr",
        "02-head",
        "03-report",
        "04-size",
        "05-missing",
        "99-valid",
    ] {
        copy_payload(&prep, &root.join(name));
    }
    for (name, field, value) in [("01-pr", "pr", "99"), ("02-head", "head_sha", "bad")] {
        let path = root.join(name).join("meta.json");
        let mut meta: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        meta[field] = json!(value);
        fs::write(path, meta.to_string()).unwrap();
    }
    fs::write(root.join("03-report/report.json"), r#"{"groups": false}"#).unwrap();
    fs::File::create(root.join("04-size/blank.txt"))
        .unwrap()
        .set_len(PAYLOAD_LIMIT + 1)
        .unwrap();
    fs::remove_file(root.join("05-missing/meta.json")).unwrap();
    let mut post = post_world(
        "collection-validation-post",
        &prep,
        "workflow_run",
        &[("payload", "payloads")],
    );
    assert_eq!(post.run(), 1);
    assert_eq!(post.runner().review_posts().len(), 2);
    assert_eq!(post.runner().comment_posts().len(), 2);
    let out = post
        .runner()
        .step_stdout("Review the findings on the pull request")
        .unwrap();
    for name in ["01-pr", "02-head", "03-report", "04-size", "05-missing"] {
        assert!(out.contains(name), "{out}");
    }
}

#[test]
fn post_collection_bounds_candidates_before_posting() {
    let prep = prepared("collection-limit", 2, "app::parse", "doctest");
    let root = prep.workspace_path().join("payloads");
    for i in 0..64 {
        copy_payload(&prep, &root.join(format!("{i:02}")));
    }
    let mut allowed = post_world(
        "collection-64",
        &prep,
        "workflow_run",
        &[("payload", "payloads")],
    )
    .open_prs(json!([]));
    assert_eq!(allowed.run(), 0);
    assert_eq!(posts(allowed.runner()), 0);
    copy_payload(&prep, &root.join("65"));
    let mut overflow = post_world(
        "collection-65",
        &prep,
        "workflow_run",
        &[("payload", "payloads")],
    );
    assert_eq!(overflow.run(), 1);
    assert_eq!(posts(overflow.runner()), 0);
    let out = overflow
        .runner()
        .step_stdout("Review the findings on the pull request")
        .unwrap();
    assert!(out.contains("payloads") && out.contains("64"), "{out}");
}

#[test]
fn post_collection_does_not_recurse_into_grandchildren() {
    let prep = prepared("collection-depth", 2, "app::parse", "doctest");
    let root = prep.workspace_path().join("payloads");
    copy_payload(&prep, &root.join("valid"));
    copy_payload(&prep, &root.join("incomplete/nested"));
    let mut post = post_world(
        "collection-depth-post",
        &prep,
        "workflow_run",
        &[("payload", "payloads")],
    );
    assert_eq!(post.run(), 1);
    assert_eq!(post.runner().review_posts().len(), 1);
}

#[cfg(unix)]
#[test]
fn post_collection_ignores_symlinks_and_treats_names_as_data() {
    let prep = prepared("collection-names", 2, "app::parse", "doctest");
    let root = prep.workspace_path().join("payloads");
    let name = "-$(touch OWNED)\n::error::injected";
    copy_payload(&prep, &root.join(name));
    copy_payload(&prep, &root.join(".hidden"));
    fs::write(root.join("ordinary-file"), "ignored").unwrap();
    std::os::unix::fs::symlink(
        prep.workspace_path().join("echoes-payload"),
        root.join("link"),
    )
    .unwrap();
    std::os::unix::fs::symlink("missing", root.join("broken-link")).unwrap();
    let mut post = post_world(
        "collection-names-post",
        &prep,
        "workflow_run",
        &[("payload", "payloads")],
    );
    assert_eq!(post.run(), 0);
    assert_eq!(post.runner().review_posts().len(), 2);
    assert!(!prep.workspace_path().join("OWNED").exists());
    let out = post
        .runner()
        .step_stdout("Review the findings on the pull request")
        .unwrap();
    assert!(
        !out.lines()
            .any(|line| line.starts_with("::error::injected")),
        "{out}"
    );
}

#[test]
fn post_collection_empty_or_missing_directory_names_the_root() {
    let prep = prepared("collection-empty", 2, "app::parse", "doctest");
    for name in ["empty", "missing"] {
        if name == "empty" {
            fs::create_dir_all(prep.workspace_path().join(name)).unwrap();
            fs::write(
                prep.workspace_path().join(name).join("ordinary-file"),
                "ignored",
            )
            .unwrap();
        }
        let mut post = post_world(name, &prep, "workflow_run", &[("payload", name)]);
        assert_eq!(post.run(), 1);
        assert_eq!(posts(post.runner()), 0);
        let out = post
            .runner()
            .step_stdout("Review the findings on the pull request")
            .unwrap();
        assert!(out.contains(name), "{out}");
    }
}

#[test]
fn post_collection_root_metadata_takes_precedence() {
    let prep = prepared("collection-root", 2, "app::parse", "doctest");
    fs::create_dir_all(prep.workspace_path().join("echoes-payload/invalid-child")).unwrap();
    let mut post = post_world("collection-root-post", &prep, "workflow_run", &[]);
    assert_eq!(post_status(&mut post), (0, "posted".into()));
    assert_eq!(post.runner().review_posts().len(), 1);
}

#[test]
fn s48_prepare_writes_a_payload_for_a_clean_scan() {
    let mut prep = World::pull_request("s48a")
        .inputs(&[("mode", "prepare"), ("payload", "echoes-payload")])
        .json(report(&[], &[], None, 6, 3));
    assert_eq!(prep.run(), 0, "a clean prepare run exits 0");
    let payload = prep.workspace_path().join("echoes-payload");
    for file in ["meta.json", "report.json"] {
        assert!(payload.join(file).is_file(), "{file}");
    }
    let mut post = post_world("s48b", &prep, "workflow_run", &[]);
    let (exit, status) = post_status(&mut post);
    assert_eq!(exit, 0, "a clean payload is no error");
    assert!(status.starts_with("skipped-"), "{status}");
}

/// Rewrites `key` in the payload metadata.
fn forge_meta(prepared: &World, key: &str, value: Value) {
    let path = prepared.workspace_path().join("echoes-payload/meta.json");
    let mut meta: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    meta[key] = value;
    fs::write(&path, meta.to_string()).unwrap();
}

/// Rewrites every line number of the payload report.
fn forge_lines(prepared: &World, value: &str) {
    let report = prepared.workspace_path().join("echoes-payload/report.json");
    let mut r: Value = serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
    for g in r["groups"].as_array_mut().unwrap() {
        for s in g["sites"].as_array_mut().unwrap() {
            s["line"] = json!(value);
        }
    }
    fs::write(&report, r.to_string()).unwrap();
}

fn posts(r: &Runner) -> usize {
    r.comment_posts().len() + r.review_posts().len()
}

#[test]
fn s24_post_rejects_a_forged_pull_request() {
    let prep = prepared("s24a", 2, "app::parse", "doctest");
    forge_meta(&prep, "pr", json!("99"));
    let mut post = post_world("s24b", &prep, "workflow_run", &[]);
    let exit = post.run();
    let r = post.runner();
    assert_ne!(
        exit, 0,
        "a payload naming another pull request fails the job"
    );
    assert_eq!(posts(r), 0, "nothing is posted for a forged payload");
    assert_eq!(r.status(), "rejected");
}

#[test]
fn s25_post_rejects_a_run_of_another_event() {
    let prep = prepared("s25a", 2, "app::parse", "doctest");
    let mut post = post_world("s25b", &prep, "workflow_run", &[]).run_event("push");
    let exit = post.run();
    let r = post.runner();
    assert_ne!(exit, 0, "only a pull request run can be reviewed");
    assert_eq!(posts(r), 0);
    assert_eq!(r.status(), "rejected");
}

#[test]
fn s26_post_skips_a_run_whose_head_moved_on() {
    let prep = prepared("s26a", 2, "app::parse", "doctest");
    let mut post = post_world("s26b", &prep, "workflow_run", &[]).open_prs(json!([]));
    let exit = post.run();
    let r = post.runner();
    assert_eq!(exit, 0, "a stale run is not an error");
    assert_eq!(
        posts(r),
        0,
        "nothing is posted on a pull request that moved on"
    );
    assert_eq!(r.status(), "skipped-stale");
}

#[test]
fn s27_post_computes_the_review_event() {
    let prep = prepared("s27a", 2, "app::parse", "doctest");
    forge_meta(&prep, "event", json!("APPROVE"));
    let mut post = post_world("s27b", &prep, "workflow_run", &[]);
    assert_eq!(post.run(), 0);
    let reviews = post.runner().review_posts();
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0]["p"], "repos/owner/repo/pulls/7/reviews");
    assert_eq!(reviews[0]["params"]["event"], "REQUEST_CHANGES");
    assert_eq!(
        post.runner().comment_posts()[0]["params"]["line"],
        "6",
        "the deletion takes the empty line the prepare job found after the block"
    );
}

#[test]
fn s28_post_never_hands_a_file_reference_to_gh() {
    let prep = prepared("s28a", 2, "app::parse", "doctest");
    forge_lines(&prep, "@/etc/hostname");
    let mut post = post_world("s28b", &prep, "workflow_run", &[]);
    post.run();
    for call in post.runner().gh_calls() {
        for (key, value) in call["params"].as_object().unwrap() {
            let value = value.as_str().unwrap_or_default();
            assert!(
                !value.starts_with('@') || key == "body",
                "{key}={value} reaches gh"
            );
        }
    }
}

#[test]
fn s29_post_applies_max_comments() {
    let prep = prepared("s29a", 4, "app::parse", "doctest");
    let mut post = post_world("s29b", &prep, "workflow_run", &[("max-comments", "1")]);
    assert_eq!(post.run(), 0);
    let r = post.runner();
    assert_eq!(r.comment_posts().len(), 1, "the cap holds in post mode");
    let body = r.review_posts()[0]["params"]["body"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(body.contains("2 inline copies were not posted"), "{body}");
}

#[test]
fn s30_post_ignores_text_planted_in_the_payload() {
    let prep = prepared("s30a", 2, "app::parse", "doctest");
    let dir = prep.workspace_path().join("echoes-payload");
    let planted = json!({"path": "src/c1.rs", "start_line": 3, "line": 5, "group": "1",
        "item": "app::parse", "close_line": 5, "body": "PLANTED"});
    fs::write(dir.join("comments.ndjson"), format!("{planted}\n")).unwrap();
    let mut post = post_world("s30b", &prep, "workflow_run", &[]);
    assert_eq!(post.run(), 0);
    let r = post.runner();
    assert_eq!(r.comment_posts().len(), 1);
    assert!(
        !joined_bodies(&r.body_files()).contains("PLANTED"),
        "comment text comes from the renderer, never from the payload"
    );
    let out = r
        .step_stdout("Review the findings on the pull request")
        .unwrap_or_default();
    assert!(
        !out.contains("::error file="),
        "the post run is not the pull request's run, so it annotates nothing"
    );
}

#[test]
fn s31_post_fences_report_text() {
    let fenced = "`` `app::a`b @org/team` ``";
    let kept_fenced = "`` `k`@org/kept ``";
    for kind in ["doctest", "delete", "helper-or-macro"] {
        let prep = prepared(&format!("s31a{kind}"), 2, "`app::a`b @org/team`", kind);
        let report = prep.workspace_path().join("echoes-payload/report.json");
        let mut r: Value = serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
        r["groups"][0]["sites"][0]["item"] = json!("`k`@org/kept");
        fs::write(&report, r.to_string()).unwrap();
        let mut post = post_world(&format!("s31b{kind}"), &prep, "workflow_run", &[]);
        assert_eq!(post.run(), 0);
        let summary = post.runner().summary();
        assert!(summary.contains(fenced), "{kind}: {summary}");
        if kind == "helper-or-macro" {
            let bodies = joined_bodies(&post.runner().body_files());
            assert!(bodies.contains(kept_fenced), "{bodies}");
        }
    }
}

#[test]
fn s32_post_logs_no_workflow_command() {
    let forged = "app::x\n::error::forged\n\n@org/team";
    for code in ["422", "500"] {
        let prep = prepared(&format!("s32a{code}"), 2, "app::parse", "doctest");
        let report = prep.workspace_path().join("echoes-payload/report.json");
        let mut r: Value = serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
        r["context_groups"] = json!([ctx_group(
            "C1",
            "HC",
            &[
                site("src/c0.rs", 3, 5, forged),
                site("src/c1.rs", 3, 5, forged)
            ]
        )]);
        fs::write(&report, r.to_string()).unwrap();
        let mut post = post_world(&format!("s32b{code}"), &prep, "workflow_run", &[])
            .env("SCEN_GH_FAIL", code);
        post.run();
        let r = post.runner();
        let out = r
            .step_stdout("Review the findings on the pull request")
            .unwrap_or_default();
        assert!(out.contains("forged"), "the failed site is logged: {out}");
        assert!(
            !out.lines()
                .any(|l| l.trim_start().starts_with("::error::forged")),
            "{code}: {out}"
        );
        let review = r.review_posts()[0]["params"]["body"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(review.contains("@org/team"), "{code}: {review}");
        assert!(
            !review
                .lines()
                .any(|l| l.trim_start().starts_with("@org/team")),
            "{code}: {review}"
        );
    }
}

/// The status of a post run, which must post nothing unless it is `posted`.
fn post_status(world: &mut World) -> (i32, String) {
    let exit = world.run();
    let r = world.runner();
    if r.status() != "posted" {
        assert_eq!(posts(r), 0, "only a posted run posts");
    }
    (exit, r.status())
}

#[test]
fn s33_post_outside_workflow_run_needs_the_same_pull_request() {
    let prep = prepared("s33a", 2, "app::parse", "doctest");
    let mut other = post_world("s33b", &prep, "workflow_dispatch", &[("pr-number", "8")]);
    assert_eq!(
        post_status(&mut other),
        (1, "rejected".into()),
        "the payload names 7, the job names 8"
    );
    let mut none = post_world("s33c", &prep, "workflow_dispatch", &[]);
    assert_eq!(
        post_status(&mut none),
        (1, "rejected".into()),
        "outside workflow_run the job names the pull request"
    );
    let mut moved = post_world("s33d", &prep, "workflow_dispatch", &[("pr-number", "7")])
        .head_sha(&"b".repeat(40));
    assert_eq!(
        post_status(&mut moved),
        (0, "skipped-stale".into()),
        "the pull request moved on"
    );
    let mut unreadable =
        post_world("s33f", &prep, "workflow_dispatch", &[("pr-number", "7")]).head_sha("null");
    assert_eq!(
        post_status(&mut unreadable),
        (1, "rejected".into()),
        "the pull request cannot be read"
    );
    let mut same = post_world("s33e", &prep, "workflow_dispatch", &[("pr-number", "7")]);
    assert_eq!(post_status(&mut same), (0, "posted".into()));
    assert_eq!(
        same.runner().review_posts()[0]["p"],
        "repos/owner/repo/pulls/7/reviews"
    );
}

#[test]
fn s34_post_rejects_identity_mismatches_under_workflow_run() {
    let prep = prepared("s34a", 2, "app::parse", "doctest");
    let mut named = post_world("s34b", &prep, "workflow_run", &[("pr-number", "8")]);
    assert_eq!(
        post_status(&mut named),
        (1, "rejected".into()),
        "the job names another pull request"
    );
    forge_meta(&prep, "pr", json!("99\n7"));
    let mut two_lines = post_world("s34c", &prep, "workflow_run", &[]);
    assert_eq!(
        post_status(&mut two_lines),
        (1, "rejected".into()),
        "a second line is not a second pattern"
    );
    forge_meta(&prep, "pr", json!("7"));
    forge_meta(&prep, "head_sha", json!("c".repeat(40)));
    let mut head = post_world("s34d", &prep, "workflow_run", &[]);
    assert_eq!(
        post_status(&mut head),
        (1, "rejected".into()),
        "the payload names another head"
    );
}

#[test]
fn s35_post_matches_the_head_repository_and_commit() {
    let prep = prepared("s35a", 2, "app::parse", "doctest");
    let (_, head) = prep.shas();
    for (case, sha, id) in [
        ("sha", "d".repeat(40), repo_id(false)),
        ("repo", head.clone(), 3),
    ] {
        let prs = json!([{"number": 7, "head": {"sha": sha, "label": "owner:feature", "repo": {"id": id, "full_name": "owner/repo"}}}]);
        let mut post = post_world(&format!("s35{case}"), &prep, "workflow_run", &[]).open_prs(prs);
        assert_eq!(
            post_status(&mut post),
            (0, "skipped-stale".into()),
            "{case}"
        );
    }
}

#[test]
fn s45_post_finds_the_pull_request_whatever_its_branch() {
    let prep = prepared("s45a", 2, "app::parse", "doctest");
    let (_, head) = prep.shas();
    let prs = json!([{"number": 7, "head": {"sha": head, "label": "owner:renamed", "repo": {"id": repo_id(false), "full_name": "owner/repo"}}}]);
    let mut post = post_world("s45b", &prep, "workflow_run", &[]).open_prs(prs);
    assert_eq!(post_status(&mut post), (0, "posted".into()));
}

#[test]
fn s46_post_rejects_a_run_head_that_is_not_a_commit() {
    let prep = prepared("s46a", 2, "app::parse", "doctest");
    let sha = "g".repeat(40);
    forge_meta(&prep, "head_sha", json!(sha));
    let mut post = post_world("s46b", &prep, "workflow_run", &[]).head_sha(&sha);
    assert_eq!(post_status(&mut post), (1, "rejected".into()));
}

/// The largest payload file post mode reads, in bytes.
const PAYLOAD_LIMIT: u64 = 128 << 20;

#[test]
fn s47_post_rejects_an_oversized_payload_file() {
    for file in ["meta.json", "report.json", "base-report.json", "blank.txt"] {
        let prep = prepared(&format!("s47a{file}"), 2, "app::parse", "doctest");
        let path = prep.workspace_path().join("echoes-payload").join(file);
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap()
            .set_len(PAYLOAD_LIMIT + 1)
            .unwrap();
        let mut post = post_world(&format!("s47b{file}"), &prep, "workflow_run", &[]);
        assert_eq!(post_status(&mut post), (1, "rejected".into()), "{file}");
    }
}

#[test]
fn s36_post_never_evaluates_report_numbers() {
    let prep = prepared("s36a", 2, "app::parse", "doctest");
    let marker = std::env::temp_dir().join(format!("echoes-s36-{}", std::process::id()));
    let _ = fs::remove_file(&marker);
    let path = prep.workspace_path().join("echoes-payload/report.json");
    let mut r: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    r["groups"][0]["sites"][0]["line"] = json!(format!("x[$(touch {})]", marker.display()));
    fs::write(&path, r.to_string()).unwrap();
    let mut post = post_world("s36b", &prep, "workflow_run", &[]);
    assert_eq!(post_status(&mut post), (1, "rejected".into()));
    assert!(!marker.exists(), "a report number ran as shell arithmetic");
    let prep = prepared("s36c", 2, "app::parse", "doctest");
    let mut base = report(
        &[group(
            "g1",
            "H1",
            &[doc_site("src/base.rs", 3, 5, "app::parse")],
        )],
        &[],
        None,
        1,
        1,
    );
    base["groups"][0]["sites"][0]["line"] = json!(format!("x[$(touch {})]", marker.display()));
    fs::write(
        prep.workspace_path()
            .join("echoes-payload/base-report.json"),
        base.to_string(),
    )
    .unwrap();
    let mut post = post_world("s36d", &prep, "workflow_run", &[]);
    assert_eq!(post_status(&mut post), (1, "rejected".into()));
    assert!(
        !marker.exists(),
        "a base report number ran as shell arithmetic"
    );
}

#[test]
fn s37_post_links_only_to_a_commit() {
    let prep = prepared("s37a", 2, "app::parse", "doctest");
    let dir = prep.workspace_path().join("echoes-payload");
    let base = report(
        &[group(
            "g1",
            "H1",
            &[doc_site("src/base.rs", 3, 5, "app::parse")],
        )],
        &[],
        None,
        1,
        1,
    );
    fs::write(dir.join("base-report.json"), base.to_string()).unwrap();
    forge_meta(&prep, "base_sha", json!("x) [phish](https://evil.example/"));
    let mut post = post_world("s37b", &prep, "workflow_run", &[]);
    assert_eq!(post_status(&mut post), (0, "posted".into()));
    let bodies = joined_bodies(&post.runner().body_files());
    assert!(
        bodies.contains("`src/base.rs:3-5`"),
        "the kept copy is named: {bodies}"
    );
    assert!(!bodies.contains("evil.example"), "{bodies}");
}

#[test]
fn s38_post_encodes_paths_in_links() {
    let prep = prepared("s38a", 3, "app::parse", "doctest");
    let kept = "src/x) [phish](https://evil.example/y.rs";
    let unanchored = "src/z) [phish](https://evil.example/w.rs";
    let report = prep.workspace_path().join("echoes-payload/report.json");
    let mut r: Value = serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
    r["groups"][0]["sites"][0]["file"] = json!(kept);
    r["groups"][0]["sites"][2]["file"] = json!(unanchored);
    r["groups"][0]["sites"][2]["end"] = Value::Null;
    fs::write(&report, r.to_string()).unwrap();
    let mut post =
        post_world("s38b", &prep, "workflow_run", &[]).changed(&[kept, "src/c1.rs", unanchored]);
    assert_eq!(post_status(&mut post), (0, "posted".into()));
    let bodies = joined_bodies(&post.runner().body_files());
    assert!(bodies.contains("/src/x%29%20%5Bphish%5D"), "{bodies}");
    assert!(!bodies.contains("/src/x) "), "{bodies}");
    let review = post.runner().review_posts()[0]["params"]["body"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(review.contains("/src/z%29%20%5Bphish%5D"), "{review}");
    assert!(!review.contains("/src/z) "), "{review}");
}

#[test]
fn s39_review_fences_candidate_text_and_checks_links() {
    let local = site("src/parser.rs", 3, 6, "app::parser::selected");
    let ctx = ctx_group(
        "C1",
        "HC",
        &[
            site("src/a.rs", 3, 6, "c`d"),
            site("src/parser.rs", 10, 13, "app::other"),
        ],
    );
    let mut world = World::pull_request("s39")
        .json(report(
            &[],
            &[ctx],
            Some(&reuse(
                &[reuse_group(
                    "R1",
                    &local,
                    &[
                        Cand::new("a`b", "1.2.3", 1, "direct::helper", "reuse-api")
                            .source_url("https://evil.example/x) [y](https://evil.example/z")
                            .existing_issue("javascript:alert(1)", "open")
                            .lookup_warning("`x`\n@org/team")
                            .build(),
                        {
                            let mut c =
                                Cand::new("second", "1.0.0", 1, "direct::helper", "reuse-api")
                                    .issue_url("https://evil.example/q) x")
                                    .build();
                            c["depth"] = json!("1 [phish](https://evil.example)");
                            c["conditions"] = json!(["f`g"]);
                            c
                        },
                        Cand::new("third", "1.0.0", 1, "direct::helper", "reuse-api")
                            .existing_issue("https://github.com/up/x/issues/1", "<b>x</b>")
                            .build(),
                    ],
                )],
                Some(coverage(true, 1, 1, 0, &[], &[])),
            )),
            6,
            3,
        ))
        .changed(&["src/a.rs", "src/parser.rs"]);
    world.write_file("src/a.rs", SRC_LIB);
    world.write_file("src/parser.rs", SRC_LIB);
    assert_eq!(world.run(), 0);
    let bodies = joined_bodies(&world.runner().body_files());
    assert!(bodies.contains("``a`b 1.2.3``"), "{bodies}");
    assert!(bodies.contains("`` `x` @org/team ``"), "{bodies}");
    assert!(bodies.contains("``c`d``"), "{bodies}");
    assert!(!bodies.contains("evil.example"), "{bodies}");
    assert!(!bodies.contains("javascript:"), "{bodies}");
    assert!(bodies.contains("(needs ``f`g``)"), "{bodies}");
    assert!(
        bodies.contains("[existing request unknown](https://github.com/up/x/issues/1)"),
        "{bodies}"
    );
}

/// A marker path a forged command substitution would create.
fn marker(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("echoes-{name}-{}", std::process::id()));
    let _ = fs::remove_file(&path);
    path
}

#[test]
fn s40_post_reads_one_report_document() {
    let prep = prepared("s40a", 2, "app::parse", "doctest");
    let mark = marker("s40");
    let evil = json!({"groups": [{"hash": "h", "kind": "doctest", "sites": [
        {"file": "src/c0.rs", "line": 1, "end": format!("a[$(touch {})]", mark.display()), "item": "a"},
        {"file": "src/c1.rs", "line": 10, "end": 12, "item": "b"}]}]});
    let report = prep.workspace_path().join("echoes-payload/report.json");
    fs::write(&report, format!("{evil}\n{{\"groups\":[]}}\n")).unwrap();
    let mut post = post_world("s40b", &prep, "workflow_run", &[]);
    assert_eq!(post_status(&mut post), (1, "rejected".into()));
    assert!(
        !mark.exists(),
        "the first document reached shell arithmetic"
    );
}

#[test]
fn s41_post_keeps_columns_with_empty_items() {
    let prep = prepared("s41a", 2, "", "doctest");
    let mut post = post_world("s41b", &prep, "workflow_run", &[]);
    assert_eq!(post_status(&mut post), (0, "posted".into()));
    let r = post.runner();
    assert_eq!(r.comment_posts()[0]["params"]["path"], "src/c1.rs");
    let bodies = joined_bodies(&r.body_files());
    assert!(
        bodies.contains("appears 2 times in this scan, this is copy 2"),
        "{bodies}"
    );
    let prep = prepared("s41c", 2, "", "doctest");
    let mark = marker("s41");
    let report = prep.workspace_path().join("echoes-payload/report.json");
    let mut r: Value = serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
    r["groups"][0]["kind"] = json!(format!("a[$(touch {})]", mark.display()));
    fs::write(&report, r.to_string()).unwrap();
    fs::write(
        prep.workspace_path()
            .join("echoes-payload/base-report.json"),
        "{\"groups\":[]}",
    )
    .unwrap();
    forge_meta(&prep, "base_sha", json!("a".repeat(40)));
    let mut post = post_world("s41d", &prep, "workflow_run", &[]);
    assert_eq!(
        post_status(&mut post),
        (0, "posted".into()),
        "an unknown kind still renders"
    );
    assert!(!mark.exists(), "a report value reached shell arithmetic");
}

#[test]
fn s42_post_rejects_numbers_the_renderer_cannot_use() {
    let prep = prepared("s42a", 2, "app::parse", "doctest");
    let report = prep.workspace_path().join("echoes-payload/report.json");
    let mut r: Value = serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
    r["groups"][0]["sites"][0]["end"] = json!(1e30);
    fs::write(&report, r.to_string()).unwrap();
    let mut post = post_world("s42b", &prep, "workflow_run", &[]);
    assert_eq!(
        post_status(&mut post),
        (1, "rejected".into()),
        "an exponent is no line"
    );
    let prep = prepared("s42c", 2, "app::parse", "doctest");
    let report = prep.workspace_path().join("echoes-payload/report.json");
    let mut r: Value = serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
    let mut open = site("src/c1.rs", 3, 5, "app::ctx");
    open["end"] = Value::Null;
    r["context_groups"] = json!([ctx_group(
        "C1",
        "HC",
        &[site("src/c0.rs", 3, 5, "app::ctx"), open]
    )]);
    fs::write(&report, r.to_string()).unwrap();
    let mut post = post_world("s42d", &prep, "workflow_run", &[]);
    assert_eq!(
        post_status(&mut post),
        (1, "rejected".into()),
        "a context block has an end"
    );
}

#[test]
fn s43_post_rejects_what_it_cannot_check() {
    let prep = prepared("s43a", 2, "app::parse", "doctest");
    let mut dry = post_world("s43b", &prep, "workflow_run", &[("dry-run", "true")]);
    assert_eq!(
        post_status(&mut dry),
        (1, "rejected".into()),
        "dry-run prints raw report text"
    );
    let mut files = post_world("s43c", &prep, "workflow_run", &[]).env("SCEN_GH_FILES_FAIL", "1");
    assert_eq!(
        post_status(&mut files),
        (1, "rejected".into()),
        "a failed diff listing is no clean diff"
    );
    for (i, meta) in ["[1, 2, 3]", "\"hello\"", "7", "{"].into_iter().enumerate() {
        fs::write(prep.workspace_path().join("echoes-payload/meta.json"), meta).unwrap();
        let mut post = post_world(&format!("s43d{i}"), &prep, "workflow_run", &[]);
        assert_eq!(
            post_status(&mut post),
            (1, "rejected".into()),
            "meta.json = {meta}"
        );
    }
}

/// Sets the value at a JSON pointer, adding the last key when the object lacks it.
fn set_at(doc: &mut Value, pointer: &str, value: Value) {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    let target = doc.pointer_mut(parent).unwrap();
    match target {
        Value::Array(items) => items[key.parse::<usize>().unwrap()] = value,
        Value::Object(map) => {
            map.insert(key.to_string(), value);
        }
        _ => panic!("{pointer} has no container"),
    }
}

#[test]
fn s44_post_rejects_every_shape_the_renderer_cannot_read() {
    let prep = prepared("s44a", 2, "app::parse", "doctest");
    let report = prep.workspace_path().join("echoes-payload/report.json");
    let mut valid: Value = serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
    valid["context_groups"] = json!([ctx_group(
        "C1",
        "HC",
        &[
            site("src/c0.rs", 3, 5, "app::ctx"),
            site("src/c1.rs", 3, 5, "app::ctx")
        ]
    )]);
    valid["reuse"] = reuse(
        &[reuse_group(
            "R1",
            &site("src/c1.rs", 3, 5, "app::local"),
            &[Cand::new("direct", "1.2.3", 1, "direct::helper", "reuse-api").build()],
        )],
        Some(coverage(true, 1, 1, 0, &[], &[])),
    );
    let run = |name: &str, doc: &Value| {
        fs::write(&report, doc.to_string()).unwrap();
        post_status(&mut post_world(name, &prep, "workflow_run", &[]))
    };
    assert_eq!(
        run("s44ok", &valid),
        (0, "posted".into()),
        "the unmodified report posts"
    );
    let cases = [
        ("/groups", json!({})),
        ("/groups/0", json!("x")),
        ("/groups/0/hash", json!(1)),
        ("/groups/0/sites", json!([])),
        ("/groups/0/sites/0", json!("x")),
        ("/groups/0/sites/0/file", json!(1)),
        ("/groups/0/sites/0/item", json!(1)),
        ("/groups/0/sites/0/line", json!(0)),
        ("/groups/0/sites/0/line", json!(1.5)),
        ("/groups/0/sites/0/line", json!(2_147_483_648_u64)),
        ("/groups/0/sites/0/line", json!("3")),
        ("/groups/0/sites/0/end", json!("5")),
        ("/groups/0/sites/0/self_type", json!(1)),
        ("/context_groups", json!({})),
        ("/context_groups/0", json!(1)),
        ("/context_groups/0/hash", json!(1)),
        ("/context_groups/0/sites", json!([])),
        ("/context_groups/0/sites/1/end", Value::Null),
        ("/reuse", json!([])),
        ("/reuse/groups", json!({})),
        ("/reuse/groups/0", json!(1)),
        ("/reuse/groups/0/hash", json!(1)),
        ("/reuse/groups/0/local", json!("x")),
        ("/reuse/groups/0/local/end", Value::Null),
        ("/reuse/groups/0/candidates", json!({})),
        ("/reuse/groups/0/candidates/0", json!("x")),
        ("/reuse/groups/0/candidates/0/conditions", json!("x")),
        ("/reuse/groups/0/candidates/0/site", json!("x")),
        ("/reuse/groups/0/candidates/0/existing_issue", json!("x")),
    ];
    for (i, (pointer, value)) in cases.into_iter().enumerate() {
        let mut doc = valid.clone();
        set_at(&mut doc, pointer, value.clone());
        assert_eq!(
            run(&format!("s44c{i}"), &doc),
            (1, "rejected".into()),
            "{pointer} = {value}"
        );
    }
}

fn scan_token(r: &Runner) -> String {
    r.scan_calls()[0]
        .get("token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[test]
fn s21_selected_repositories() {
    let mut world = World::new("s21").inputs(&[("repositories", "o/a\n  O/b \n\n")]);
    let nested = world.workspace_path().join("crates/app");
    world.set_meta_root(nested);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "a clean selection scan exits 0");
    let scan = r.scan_args(0);
    assert_eq!(
        &scan[scan.len() - 4..],
        ["--repository", "o/a", "--repository", "O/b"],
        "each nonblank line becomes one trimmed repository"
    );
    assert!(
        !scan
            .iter()
            .any(|a| a == "--github" || a == "--organization"),
        "{scan:?}"
    );
    assert_eq!(
        scan_token(r),
        "workflow-token",
        "the workflow token authenticates the listing"
    );
    let prefix = fs::read_to_string(Path::new(r.run_temp()).join("echoes-prefix")).unwrap();
    assert_eq!(
        prefix, "",
        "selected repositories are not under the local workspace"
    );
}

#[test]
fn s22_selected_organizations_keep_the_job_token() {
    let mut world = World::new("s22")
        .inputs(&[("organizations", "acme\nwidgets")])
        .env("GH_TOKEN", "pat");
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "a clean selection scan exits 0");
    let scan = r.scan_args(0);
    assert_eq!(
        &scan[scan.len() - 4..],
        ["--organization", "acme", "--organization", "widgets"]
    );
    assert_eq!(
        scan_token(r),
        "pat",
        "a token the job sets reaches private repositories"
    );
}

#[test]
fn s49_selected_organizations_and_repositories_join() {
    let mut world = World::new("s49").inputs(&[("organizations", "acme"), ("repositories", "o/a")]);
    let exit = world.run();
    let r = world.runner();
    assert_eq!(exit, 0, "a clean selection scan exits 0");
    let scan = r.scan_args(0);
    assert_eq!(
        &scan[scan.len() - 4..],
        ["--organization", "acme", "--repository", "o/a"]
    );
}

#[test]
fn s23_selection_is_gate_only() {
    for (case, extra) in [
        ("pr", ("pr-number", "7")),
        ("dry", ("dry-run", "true")),
        ("prepare", ("mode", "prepare")),
        ("post", ("mode", "post")),
    ] {
        let mut world =
            World::new(&format!("s23-{case}")).inputs(&[("repositories", "o/a"), extra]);
        let exit = world.run();
        let r = world.runner();
        assert_ne!(exit, 0, "{case}");
        assert!(r.scan_calls().is_empty(), "{case} scans nothing");
        assert!(r.gh_calls().is_empty(), "{case} calls no API");
    }
}

/// The hidden key a comment of `scope` carries for `key`, both `@uri` encoded.
fn thread_marker(scope: &str, key: &str) -> String {
    format!("<!-- echoes:v1 scope={scope} key={key} -->")
}

/// The hidden mark a review of `scope` carries.
fn review_marker(scope: &str) -> String {
    format!("<!-- echoes:v1 scope={scope} review -->")
}

const CONTEXT_NOTE: &str =
    "This context is repeated in this scan. A shared helper can carry it once.";
const LEGACY_OOR: &str = "These duplicates are not on lines GitHub shows in this pull request diff, the inline comment cannot attach there. The links point at the copies on the head commit.";
const ECHOES: &str = "[echoes](https://github.com/LucaCappelletti94/cargo-echoes)";
const BOT: &str = "github-actions[bot]";
const DUP_KEY: &str = "doctest%3AH1%3Asrc%2Fdup.rs%3Aapp%3A%3Aparse";

/// A git world with a foreign workspace under `sub`, where `src` belongs to the root.
fn cleanup_world(name: &str) -> World {
    World::pull_request(name)
        .git()
        .extra_base("sub/Cargo.toml", "[workspace]\n")
        .extra_base("sub/src/lib.rs", SRC_LIB)
        .changed(&["src/lib.rs"])
}

#[test]
fn s60_clean_head_resolves_gone_threads_and_dismisses_reviews() {
    let gone = format!(
        "{CONTEXT_NOTE}\n\n{}",
        thread_marker(".", "context%3AC9%3Asrc%2Fold.rs%3Aapp%3A%3Aold")
    );
    let other = format!(
        "{CONTEXT_NOTE}\n\n{}",
        thread_marker("sub", "context%3AC9%3Asub%2Fsrc%2Flib.rs%3Ax")
    );
    let mut world = cleanup_world("s60")
        .threads(json!([
            thread("t1", false, "src/old.rs", None, "github-actions", &gone),
            thread(
                "t2",
                false,
                "sub/src/lib.rs",
                Some(3),
                "github-actions",
                &other
            ),
            thread("t3", true, "src/old.rs", None, "github-actions", &gone),
            thread(
                "t4",
                false,
                "src/lib.rs",
                Some(3),
                "github-actions",
                CONTEXT_NOTE
            ),
            thread(
                "t5",
                false,
                "sub/src/lib.rs",
                Some(3),
                "github-actions",
                CONTEXT_NOTE
            ),
            thread(
                "t6",
                false,
                "src/lib.rs",
                Some(3),
                "github-actions",
                "Clippy found a warning"
            ),
            thread("t7", false, "src/lib.rs", Some(3), "luca", CONTEXT_NOTE),
        ]))
        .reviews(json!([
            review(
                101,
                "CHANGES_REQUESTED",
                BOT,
                &format!("x\n\n{}", review_marker("."))
            ),
            review(102, "CHANGES_REQUESTED", BOT, ""),
            review(
                103,
                "CHANGES_REQUESTED",
                BOT,
                &format!("{LEGACY_OOR}\n\n- group context  `sub/src/lib.rs:3`  `x`")
            ),
            review(104, "COMMENTED", BOT, &review_marker(".")),
            review(108, "COMMENTED", BOT, ""),
            review(105, "CHANGES_REQUESTED", BOT, &review_marker("sub")),
            review(106, "CHANGES_REQUESTED", "luca", ""),
            review(
                107,
                "CHANGES_REQUESTED",
                BOT,
                &format!("{LEGACY_OOR}\n\n- group 1  `src/lib.rs:3`  `app::parse`")
            ),
        ]));
    assert_eq!(world.run(), 0, "a clean head exits 0");
    let r = world.runner();
    assert_eq!(
        r.resolved_threads(),
        ["t1", "t4"],
        "only this workspace's open echoes threads whose finding is gone are resolved"
    );
    let mut dismissed = r.dismissed_reviews();
    dismissed.sort();
    assert_eq!(
        dismissed,
        ["101", "102", "107"],
        "only this workspace's requests for changes are dismissed"
    );
    assert_clean_summaries(r);
    assert!(
        r.comment_posts().is_empty() && r.review_posts().is_empty(),
        "a clean head posts nothing"
    );
}

/// Asserts a clean head dismissed with the clean message and rewrote only review 104.
fn assert_clean_summaries(r: &Runner) {
    assert!(
        r.dismissal_messages()
            .iter()
            .all(|m| m.starts_with("No echoes finding of this workspace remains at ")),
        "a clean head dismisses with the clean message: {:?}",
        r.dismissal_messages()
    );
    let rewritten = r.rewritten_reviews();
    assert_eq!(
        rewritten
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        ["104"],
        "only this workspace's comment-only summary is rewritten, never an inline comment's review"
    );
    assert!(
        rewritten[0].1.starts_with(&format!(
            "No {ECHOES} finding of this workspace remains at `"
        )) && rewritten[0].1.ends_with(&review_marker(".")),
        "the rewritten summary says no finding remains and keeps its mark: {}",
        rewritten[0].1
    );
}

#[test]
fn s61_threaded_finding_is_not_posted_again() {
    let mut world = World::pull_request("s61")
        .git()
        .json(report(
            &[group(
                "g1",
                "H1",
                &[
                    doc_site("src/lib.rs", 3, 5, "app::parse"),
                    doc_site("src/dup.rs", 3, 5, "app::parse"),
                ],
            )],
            &[],
            None,
            6,
            3,
        ))
        .base_json(report(
            &[group(
                "g1",
                "H1",
                &[doc_site("src/lib.rs", 3, 5, "app::parse")],
            )],
            &[],
            None,
            1,
            1,
        ))
        .changed(&["Cargo.toml", "src/lib.rs", "src/dup.rs"])
        .threads(json!([thread(
            "t1",
            false,
            "src/dup.rs",
            None,
            "github-actions",
            &format!("x\n\n{}", thread_marker(".", DUP_KEY))
        )]))
        // The listing already holds the review this run posts, id 9001, which must stay.
        .reviews(json!([
            review(201, "CHANGES_REQUESTED", BOT, &review_marker(".")),
            review(
                202,
                "COMMENTED",
                BOT,
                &format!("x\n\n{}", review_marker("."))
            ),
            review(9001, "CHANGES_REQUESTED", BOT, &review_marker(".")),
        ]));
    assert_eq!(world.run(), 0, "the review run exits 0");
    let r = world.runner();
    assert!(
        r.comment_posts().is_empty(),
        "a finding with a thread is not posted again"
    );
    assert!(
        r.resolved_threads().is_empty(),
        "the finding is still present, so its thread stays open"
    );
    let reviews = r.review_posts();
    assert_eq!(
        reviews.len(),
        1,
        "the request for changes is renewed on the new head"
    );
    assert_eq!(
        reviews[0].pointer("/params/event").and_then(Value::as_str),
        Some("REQUEST_CHANGES")
    );
    assert_renewed_summary(r, &reviews[0]);
    assert_eq!(
        r.dismissed_reviews(),
        ["201"],
        "the superseded request is dismissed, the new one stays"
    );
    let calls = r.gh_calls();
    let posted = calls
        .iter()
        .position(|c| c["m"] == "POST" && c["p"].as_str().is_some_and(|p| p.ends_with("/reviews")));
    let dismissed = calls.iter().position(|c| c["m"] == "PUT");
    assert!(
        posted < dismissed,
        "the new review is posted before the old one is dismissed"
    );
}

/// Asserts the renewed review's lead and footer and the superseded rewrite of review 202.
fn assert_renewed_summary(r: &Runner, review: &Value) {
    let body = review
        .pointer("/params/body")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        body.starts_with(&format!(
            "{ECHOES} still finds the duplicates it reported on earlier commits"
        )) && body.contains(&review_marker(".")),
        "the renewed review names echoes in its lead and carries its workspace mark: {body}"
    );
    assert!(
        !body.contains(&format!("{ECHOES} ·")),
        "the footer does not repeat the echoes link: {body}"
    );
    let rewritten = r.rewritten_reviews();
    assert!(
        rewritten.len() == 1
            && rewritten[0].0 == "202"
            && rewritten[0]
                .1
                .starts_with(&format!("Superseded by the {ECHOES} review of `")),
        "the earlier comment-only summary is marked superseded: {rewritten:?}"
    );
}

#[test]
fn s62_legacy_thread_covering_a_finding_is_kept() {
    let legacy = "This pull request adds 2 copies of the same doctest, echoes suggests keeping the first added copy.";
    let mut world = World::pull_request("s62")
        .inputs(&[("only-new", "false")])
        .git()
        .json(report(
            &[group(
                "g1",
                "H1",
                &[
                    doc_site("src/lib.rs", 3, 5, "app::parse"),
                    doc_site("src/dup.rs", 3, 5, "app::parse"),
                ],
            )],
            &[],
            None,
            6,
            3,
        ))
        .changed(&["Cargo.toml", "src/lib.rs", "src/dup.rs"])
        .threads(json!([
            thread("t1", false, "src/dup.rs", Some(4), "github-actions", legacy),
            thread("t2", false, "src/lib.rs", None, "github-actions", legacy),
        ]));
    assert_eq!(world.run(), 0, "the review run exits 0");
    let r = world.runner();
    assert!(
        r.comment_posts().is_empty(),
        "a legacy thread on the copy's lines stands for the finding"
    );
    assert_eq!(
        r.resolved_threads(),
        ["t2"],
        "the legacy thread covering no finding is resolved"
    );
}

#[test]
fn s63_context_out_of_range_links_and_requests_nothing() {
    let mut world = World::pull_request("s63")
        .inputs(&[("context-select", "crate:app")])
        .git()
        .json(report(
            &[],
            &[ctx_group(
                "c9f8",
                "C1",
                &[
                    site("src/parser.rs", 3, 6, "app::parser::selected"),
                    site("src/mod.rs", 10, 13, "app::other::selected"),
                ],
            )],
            None,
            6,
            3,
        ))
        .base_json(report(&[], &[], None, 6, 3))
        .changed(&["Cargo.toml", "src/lib.rs", "src/parser.rs", "src/mod.rs"])
        .extra_base("src/mod.rs", SRC_PARSER)
        .env("SCEN_GH_FAIL", "422");
    assert_eq!(world.run(), 0, "the review run exits 0");
    let r = world.runner();
    let reviews = r.review_posts();
    assert_eq!(
        reviews
            .first()
            .and_then(|c| c.pointer("/params/event"))
            .and_then(Value::as_str),
        Some("COMMENT"),
        "a context that cannot attach inline requests no changes"
    );
    let body = reviews
        .first()
        .and_then(|c| c.pointer("/params/body"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        body.contains("[`src/mod.rs:10-13`](https://github.com/owner/repo/blob/")
            && body.contains("/src/mod.rs#L10-L13)"),
        "the out-of-range copy links its lines: {body}"
    );
    assert!(
        body.starts_with(&format!("{ECHOES} found repeated code in this pull request.\n\nThese duplicates are on lines this diff does not show, so they have no inline comment. The links point at the head commit."))
            && !body.contains(&format!("{ECHOES} ·")),
        "the summary names echoes once, in its lead: {body}"
    );
}

#[test]
fn s64_refused_resolution_leaves_a_notice() {
    let gone = format!(
        "{CONTEXT_NOTE}\n\n{}",
        thread_marker(".", "context%3AC9%3Asrc%2Fold.rs%3Aapp%3A%3Aold")
    );
    let mut world = cleanup_world("s64")
        .threads(json!([thread(
            "t1",
            false,
            "src/old.rs",
            None,
            "github-actions",
            &gone
        )]))
        .env("SCEN_GH_RESOLVE_403", "1");
    assert_eq!(world.run(), 0, "a refused resolution does not fail the run");
    let out = world
        .runner()
        .step_stdout("Review the findings on the pull request")
        .unwrap_or_default();
    assert!(
        out.contains("contents: write"),
        "the notice names the missing permission: {out}"
    );
}

/// The context world of `s06`, whose one inline finding is keyed by `CTX_KEY`.
fn advice_world(name: &str) -> World {
    World::pull_request(name)
        .inputs(&[("context-select", "crate:app")])
        .git()
        .json(report(
            &[],
            &[ctx_group(
                "c9f8",
                "C1",
                &[
                    site("src/parser.rs", 3, 6, "app::parser::selected"),
                    site("src/mod.rs", 10, 13, "app::other::selected"),
                ],
            )],
            None,
            6,
            3,
        ))
        .base_json(report(&[], &[], None, 6, 3))
        .changed(&["Cargo.toml", "src/lib.rs", "src/parser.rs", "src/mod.rs"])
        .extra_base("src/mod.rs", SRC_PARSER)
}

const CTX_KEY: &str = "context%3AC1%3Asrc%2Fmod.rs%3Aapp%3A%3Aother%3A%3Aselected";

/// The earlier echoes summaries of this workspace, one of each state.
fn earlier_summaries() -> Value {
    json!([
        review(301, "CHANGES_REQUESTED", BOT, &review_marker(".")),
        review(
            302,
            "COMMENTED",
            BOT,
            &format!("x\n\n{}", review_marker("."))
        ),
    ])
}

/// Asserts the earlier summaries were cleaned up while advice findings remain.
fn assert_advice_cleanup(r: &Runner) {
    assert!(
        r.review_posts().is_empty(),
        "advice findings need no summary review"
    );
    assert_eq!(
        r.dismissed_reviews(),
        ["301"],
        "the earlier request is dismissed"
    );
    assert!(
        r.dismissal_messages()[0]
            .starts_with("No echoes finding of this workspace requests changes at "),
        "the dismissal says nothing requests changes: {:?}",
        r.dismissal_messages()
    );
    let rewritten = r.rewritten_reviews();
    assert!(
        rewritten.len() == 1
            && rewritten[0].0 == "302"
            && rewritten[0].1.starts_with(&format!(
                "The open {ECHOES} threads carry the findings that remain at `"
            ))
            && rewritten[0].1.ends_with(&review_marker(".")),
        "the earlier comment-only summary points at the open threads: {rewritten:?}"
    );
}

#[test]
fn s70_new_advice_cleans_up_without_a_summary() {
    let mut world = advice_world("s70").reviews(earlier_summaries());
    assert_eq!(world.run(), 0, "the advice run exits 0");
    let r = world.runner();
    assert_eq!(r.comment_posts().len(), 1, "the advice is posted inline");
    assert_eq!(r.status(), "posted", "the inline advice counts as posted");
    assert_advice_cleanup(r);
}

#[test]
fn s71_remaining_advice_cleans_up_without_a_summary() {
    let mut world = advice_world("s71")
        .threads(json!([thread(
            "t1",
            false,
            "src/mod.rs",
            None,
            "github-actions",
            &format!("x\n\n{}", thread_marker(".", CTX_KEY))
        )]))
        .reviews(earlier_summaries());
    assert_eq!(world.run(), 0, "the remaining-advice run exits 0");
    let r = world.runner();
    assert!(
        r.comment_posts().is_empty() && r.resolved_threads().is_empty(),
        "the threaded advice is neither posted again nor resolved"
    );
    assert_advice_cleanup(r);
}

#[test]
fn s72_comment_summary_rewrite_is_idempotent_and_refusal_is_reported() {
    let head = "1111111111111111111111111111111111111111";
    let settled = format!(
        "No {ECHOES} finding of this workspace remains at `{head}`.\n\n{}",
        review_marker(".")
    );
    let mut world = World::pull_request("s72a")
        .json(report(&[], &[], None, 6, 3))
        .head_sha(head)
        .reviews(json!([review(401, "COMMENTED", BOT, &settled)]));
    assert_eq!(world.run(), 0, "a clean run exits 0");
    assert!(
        world.runner().rewritten_reviews().is_empty(),
        "a summary already saying the same is left alone"
    );
    let mut refused = World::pull_request("s72b")
        .json(report(&[], &[], None, 6, 3))
        .head_sha(head)
        .reviews(json!([review(402, "COMMENTED", BOT, &review_marker("."))]))
        .env("SCEN_GH_REVIEW_EDIT_403", "1");
    assert_eq!(refused.run(), 0, "a refused rewrite does not fail the run");
    let out = refused
        .runner()
        .step_stdout("Review the findings on the pull request")
        .unwrap_or_default();
    assert!(
        out.contains("rewriting a review needs pull-requests: write"),
        "the notice names the missing permission: {out}"
    );
}

#[test]
fn s65_post_mode_cleans_up_the_payload_scope() {
    let mut prep = World::pull_request("s65a")
        .inputs(&[("mode", "prepare"), ("payload", "echoes-payload")])
        .json(report(&[], &[], None, 6, 3));
    assert_eq!(prep.run(), 0, "a clean prepare run exits 0");
    let meta: Value = serde_json::from_str(
        &fs::read_to_string(prep.workspace_path().join("echoes-payload/meta.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        meta["scope"], ".",
        "the payload names the workspace it scanned"
    );
    assert!(
        meta["owned_dirs"]
            .as_array()
            .is_some_and(|d| d.contains(&json!("."))),
        "the payload lists the workspace root as owned: {meta}"
    );
    let mine = format!(
        "x\n\n{}",
        thread_marker(".", "context%3AC9%3Asrc%2Fold.rs%3Aapp%3A%3Aold")
    );
    let theirs = format!(
        "x\n\n{}",
        thread_marker("sub", "context%3AC9%3Asub%2Fold.rs%3Aapp%3A%3Aold")
    );
    let mut post = post_world("s65b", &prep, "workflow_run", &[]).threads(json!([
        thread("t1", false, "src/old.rs", None, "github-actions", &mine),
        thread("t2", false, "sub/old.rs", None, "github-actions", &theirs),
    ]));
    let (exit, _) = post_status(&mut post);
    assert_eq!(exit, 0, "the clean post run exits 0");
    assert_eq!(
        post.runner().resolved_threads(),
        ["t1"],
        "the post job cleans up the payload's workspace only"
    );
    forge_meta(&prep, "scope", json!("sub --> <b>"));
    let mut forged = post_world("s65c", &prep, "workflow_run", &[]).threads(json!([thread(
        "t1",
        false,
        "src/old.rs",
        None,
        "github-actions",
        &mine
    )]));
    let (exit, status) = post_status(&mut forged);
    assert_eq!(
        (exit, status.as_str()),
        (1, "rejected"),
        "a malformed scope rejects the payload"
    );
    assert!(
        forged.runner().resolved_threads().is_empty(),
        "a rejected payload touches no thread"
    );
}

#[test]
fn s66_scan_summary_escapes_report_text() {
    let forged = "app::x`<b>@org/team";
    let local = site("src/parser.rs", 3, 6, forged);
    let rep = report(
        &[],
        &[ctx_group(
            "c1",
            "C1",
            &[local.clone(), site("src/mod.rs", 10, 13, forged)],
        )],
        Some(&reuse(
            &[reuse_group(
                "R1",
                &local,
                &[Cand::new("direct", "1.2.3", 1, forged, "reuse-api")
                    .source_url("javascript:alert(1)")
                    .build()],
            )],
            Some(coverage(true, 1, 1, 0, &[], &[])),
        )),
        6,
        3,
    );
    let mut world = World::pull_request("s66")
        .inputs(&[("context-dependency-depth", "2")])
        .json(rep);
    world.run();
    let summary = world.runner().summary();
    assert!(
        summary.contains("``app::x`<b>@org/team``"),
        "report items render as code spans in the run summary: {summary}"
    );
    assert!(
        !summary.contains("javascript:"),
        "a source url that is not https stays out of the run summary"
    );
}

#[test]
fn s67_base_scan_takes_the_head_scan_flags() {
    let mut world = World::pull_request("s67")
        .inputs(&[
            ("functions", "true"),
            ("all-targets", "true"),
            ("context-blocks", "true"),
            ("context-dependency-depth", "2"),
            ("context-dev-dependencies", "true"),
            ("exclude", "benches/**\nsrc/gen.rs"),
        ])
        .git()
        .json(report(&[], &[], None, 6, 3))
        .base_json(report(&[], &[], None, 1, 1));
    world.run();
    let r = world.runner();
    let without_threshold = |args: Vec<String>| {
        let at = args
            .iter()
            .position(|a| a == "--threshold")
            .expect("a threshold");
        let mut rest = args;
        rest.drain(at..at + 2);
        rest
    };
    let head = without_threshold(r.scan_args(0));
    assert!(
        head.iter().any(|a| a == "--context-dependency-depth"),
        "the head scan searches dependencies: {head:?}"
    );
    assert_eq!(
        without_threshold(r.scan_args(1)),
        head,
        "the base scan passes the head scan's flags, only the threshold differs"
    );
}

#[test]
fn s68_a_second_copy_of_a_base_context_counts_as_added() {
    let old = site("src/parser.rs", 3, 6, "app::parser::selected");
    let added = site("src/parser.rs", 12, 15, "app::parser::selected");
    let mut world = World::pull_request("s68")
        .inputs(&[("context-select", "crate:app")])
        .git()
        .json(report(
            &[],
            &[ctx_group("c1", "C1", &[old.clone(), added])],
            None,
            6,
            3,
        ))
        .base_json(report(&[], &[ctx_group("c1", "C1", &[old])], None, 6, 3))
        .changed(&["src/parser.rs"]);
    assert_eq!(world.run(), 0, "the review run exits 0");
    let posts = world.runner().comment_posts();
    assert_eq!(
        posts.len(),
        1,
        "the copy beyond the base's one is added by the pull request"
    );
    assert_eq!(
        posts[0]
            .pointer("/params/start_line")
            .and_then(Value::as_str),
        Some("12"),
        "the comment lands on the added copy"
    );
}

#[test]
fn s69_annotation_names_the_group_first_copy_outside_only_new() {
    let mut world = World::pull_request("s69")
        .inputs(&[("only-new", "false")])
        .git()
        .json(report(
            &[group(
                "g1",
                "H1",
                &[
                    doc_site("src/lib.rs", 3, 5, "app::parse"),
                    doc_site("src/dup.rs", 3, 5, "app::parse"),
                ],
            )],
            &[],
            None,
            6,
            3,
        ))
        .changed(&["src/dup.rs"]);
    assert_eq!(world.run(), 0, "the review run exits 0");
    let out = world
        .runner()
        .step_stdout("Review the findings on the pull request")
        .unwrap_or_default();
    let note = out
        .lines()
        .find(|l| l.starts_with("::error file=src/dup.rs"))
        .unwrap_or_default();
    assert!(
        note.contains("duplicates the doctest at src/lib.rs:3"),
        "the copy's annotation points at the group's first copy, not at itself: {note}"
    );
}
