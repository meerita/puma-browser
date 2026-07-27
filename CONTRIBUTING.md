# Contributing to Puma

Thank you for your interest in Puma. This guide covers how to set up the project, the
branch model, commit style, and how to open a pull request. All participants are
expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Getting started

Puma is a Rust workspace. It requires Rust stable.

```bash
git clone https://github.com/meerita/puma-browser
cd puma-browser
make build
```

The workspace is organized into crates under `crates/`, each with a single
responsibility. See [docs/architecture/README.md](docs/architecture/README.md) for the
crate map and the rendering pipeline.

## Branch model

```
main          Stable releases, tagged with semver (v0.1.0, v0.2.0, ...)
dev           Integration branch, all pull requests target this
feat/...      New user-facing functionality
fix/...       Bug fixes
doc/...       Documentation only
chore/...     Tooling, CI, dependency updates
refactor/...  Internal restructuring with no behavior change
```

Never commit directly to `main` or `dev`. Both branches are protected and only accept
changes through pull requests.

Branch names are lowercase and hyphen-separated, and they describe the work:
`feat/find-in-page`, `fix/redirect-loop`, `doc/readme-badges`. Avoid generic names
like `wip`, `temp`, or `fix/bug`.

## Working on a change

1. Branch from the latest `dev`:

   ```bash
   git checkout dev
   git pull origin dev
   git checkout -b feat/my-thing
   ```

2. Implement the change. Keep it focused: one logical change per commit.

3. Run the quality gates (see below) before pushing.

## Quality gates

All gates must pass before a pull request is ready for review. Run them from the repo
root:

```bash
make fmt-check    # Formatting (auto-fix with: make fmt)
make lint         # Clippy, -D warnings, all targets
make check        # Compile check
make test         # Unit and integration tests
make audit        # Dependency advisories, zero at warning or above
```

The same checks run in CI on every pull request.

## Commit style

Puma follows [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): imperative summary
```

- `type` is one of `feat`, `fix`, `doc`, `chore`, `refactor`, `test`, `perf`.
- `scope` is the crate or area affected: `core`, `network`, `html`, `css`, `layout`,
  `storage`, `privacy`, `terminal`, `mcp`, `cli`. Omit the scope for cross-cutting
  changes.
- The summary is in the imperative mood and describes what the commit does.

Examples:

```
feat(terminal): add slash command autocomplete
fix(network): handle redirect loops correctly
doc(readme): add status badges and install section
```

Keep commits atomic. Do not use placeholder messages such as `wip`, `update`, or
`changes`. Do not add co-author or AI-attribution trailers.

## Versioning

Puma uses [Semantic Versioning](https://semver.org/). The workspace `version` field in
the root `Cargo.toml` is the single source of truth. A `feat/` branch bumps the minor
version, a `fix/` branch bumps the patch version, and `chore/`, `doc/`, and `refactor/`
branches do not change the version. The bump belongs in the same commit that completes
the change.

## Opening a pull request

Push your branch and open a pull request against `dev`:

```bash
git push -u origin feat/my-thing
gh pr create --base dev --title "feat(scope): summary" --body "..."
```

A pull request needs a real title (not a copy of the branch name), a description of
what changed and why, and it must target `dev`. Work in progress is signaled by
GitHub's draft status, never by a `[WIP]` marker in the title.

## Security-sensitive changes

Any change that touches TLS, cookies, request filtering, MCP permissions, terminal
output sanitization, or URL handling requires extra care. Remote content is untrusted:
it must never reach the terminal as raw bytes, and secrets (cookie values, passwords,
tokens) must never appear in logs, `Debug` output, or MCP responses. Describe the
security impact in the pull request description.

## Reporting bugs and requesting features

Open an issue on GitHub. For bugs, include the URL or input that triggered the problem,
what you expected, what happened, and your platform. For security vulnerabilities, do
not open a public issue: contact the maintainer directly at meerita@icloud.com.

## License

By contributing to Puma, you agree that your contributions are licensed under the
[MIT License](LICENSE).
