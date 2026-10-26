# cargo-echoes

[![CI](https://github.com/LucaCappelletti94/cargo-echoes/actions/workflows/ci.yml/badge.svg)](https://github.com/LucaCappelletti94/cargo-echoes/actions/workflows/ci.yml)
[![codecov](https://codecov.io/github/LucaCappelletti94/cargo-echoes/graph/badge.svg)](https://codecov.io/github/LucaCappelletti94/cargo-echoes)
[![crates.io](https://img.shields.io/crates/v/echoes.svg)](https://crates.io/crates/echoes)
[![docs.rs](https://docs.rs/echoes/badge.svg)](https://docs.rs/echoes)

By chance two separate functions meet, and I am you and what I see is me.

`cargo echoes` finds duplicated Rust doctests, and optionally duplicated functions and code blocks, across a workspace or across every crate of a GitHub organization. The [wiki](https://github.com/LucaCappelletti94/cargo-echoes/wiki) explains each check, the options and the Action in depth.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/LucaCappelletti94/cargo-echoes/main/docs/review-dark.svg">
  <img alt="A pull request review by echoes, with an inline comment on a rewritten copy of a function and a suggestion that deletes it" src="https://raw.githubusercontent.com/LucaCappelletti94/cargo-echoes/main/docs/review-light.svg">
</picture>

## Usage

```bash
cargo install echoes
cargo echoes --functions --context-blocks
cargo echoes --organization --functions
```

| Check | Enable | Default minimum size | Set the size |
| --- | --- | --- | --- |
| Doctests | On by default | `0` tokens | `--min-tokens` |
| Functions | `--functions` | `30` weighted body tokens | `--fn-min-tokens` |
| Contexts | `--context-blocks` | `30` canonical weighted tokens | `--context-min-tokens` |

Options can also live in `.echoes.toml` at the workspace root, and `cargo echoes --help` lists them all. Leave out whole paths, such as benchmarks, with gitignore-style `exclude` globs or `--exclude`, and keep an intentional copy with a `rust,echoes` fence or a `// echoes: allow` line above the function. The wiki lists [what counts as the same code](https://github.com/LucaCappelletti94/cargo-echoes/wiki/What-counts-as-the-same-code).

`--organization` scans the default branch of every repository of a GitHub organization or user that the `GH_TOKEN` or `GITHUB_TOKEN` credential can read, private ones included, and compares functions across crates and modules. Without a name it scans the owner of the current repository, from `GITHUB_REPOSITORY_OWNER` or the `origin` remote. Repeat it to join several owners, and pass `--repository OWNER/REPO` once per repository to add repositories of any owner, alone or beside the organizations. Downloaded repositories never configure the scan or run code, so pass `--config` for options, and the exit status is `2` whenever a repository could not be analyzed, even with `--no-fail`.

## GitHub Action

```yaml
on: pull_request
jobs:
  echoes:
    runs-on: ubuntu-26.04
    permissions:
      contents: write
      pull-requests: write
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: LucaCappelletti94/cargo-echoes@v1
        with:
          pr-number: ${{ github.event.pull_request.number }}
          functions: true
```

The [action](https://github.com/marketplace/actions/cargo-echoes) reviews the pull request and suggests removing each copy it introduces, with links to report a wrong review and support echoes. On every new commit it dismisses its earlier requests for changes, updates its earlier comment-only summaries and resolves the threads of copies that are gone, which needs `contents: write`. [`action.yml`](https://github.com/LucaCappelletti94/cargo-echoes/blob/main/action.yml) lists every input, and [these two workflows](https://github.com/LucaCappelletti94/cargo-echoes/tree/main/examples/workflows) review pull requests from forks.

## Library

```rust
let report = echoes::Echoes::default()
    .functions()
    .context_blocks()
    .run(".")
    .unwrap();
println!("{}", echoes::human(&report, false));
```

Use `default-features = false, features = ["std"]` to drop the CLI. Coding agents get the same guidance from the [`echoes` skill](https://github.com/LucaCappelletti94/cargo-echoes/blob/main/skills/echoes/SKILL.md).

```bash
npx skills add LucaCappelletti94/cargo-echoes
```
