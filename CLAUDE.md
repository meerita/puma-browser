# Puma — Modern Terminal Browser

Puma is an open-source, native, text-first, scriptless web browser for the terminal.
Built in Rust. Inspired by Lynx, designed with the UX of modern CLI tools.

## What this is

A purpose-built browser for web documents:

```
HTML → Semantic document → Text layout → Terminal or MCP
```

No JavaScript. No Electron. No Chromium. Native binaries on Linux, macOS, and Windows.

The functional specification is at `docs/temp/functional-spec.md` (not committed; used as guidance during development).

## Repository layout

```
crates/
  browser-core/       Navigation controller, tabs, history, bookmarks, forms, downloads
  browser-network/    HTTP client, TLS, redirects, proxy, cookies, cache, request filtering
  browser-html/       HTML5 parser (html5ever), semantic DOM builder
  browser-css/        Reduced CSS cascade for text rendering (cssparser)
  browser-layout/     Text layout engine, terminal cell renderer, word wrapping
  browser-storage/    SQLite persistence: config, profiles, history, bookmarks, themes
  browser-privacy/    Cookie policy, privacy dashboard, session isolation
  browser-terminal/   Ratatui TUI, command system, theme engine, input handling
  browser-mcp/        MCP server (stdio), tools, resources, permissions
  browser-cli/        Binary entry point
docs/
  architecture/       Architecture notes and decisions
  process/            Development workflow
  plans/              Implementation plans (gitignored; local working artifacts)
docs/temp/            (gitignored) Working documents — functional spec, scratch notes
Cargo.toml            Workspace manifest
Makefile              Quality gates
```

## Tech stack

| Purpose | Crate |
| ------- | ----- |
| Async runtime | tokio |
| Terminal UI | ratatui + crossterm |
| HTTP client | reqwest |
| TLS | rustls |
| HTML parser | html5ever |
| CSS parsing | cssparser |
| Storage | rusqlite (SQLite, bundled) |
| Config | toml |
| Logging | tracing + tracing-subscriber |
| Unicode segmentation | unicode-segmentation |
| Unicode width | unicode-width |
| Serialization | serde + serde_json |
| Error handling | anyhow + thiserror |

## Quality gates

Run from the repository root. All must pass before opening a PR.

```bash
make fmt          # cargo fmt --all  (auto-fixes formatting)
make fmt-check    # cargo fmt --all -- --check
make lint         # cargo clippy --all-targets --all-features -- -D warnings
make check        # cargo check --all-targets --all-features
make test         # cargo test --all
make test-full    # cargo test --all --all-features
make build        # cargo build --release
```

## Git identity

All commits must use the meerita identity. The local repo is configured for this,
but verify before committing:

```bash
git config user.name   # must be: meerita
git config user.email  # must be: meerita@icloud.com
```

Never commit with the cognativ/work identity on this repo.

## Branch model

- `main` — stable releases, tagged with semver
- `dev` — integration branch; all PRs target this
- Feature branches: `feat/`, `fix/`, `doc/`, `chore/`, `refactor/`
- Flow: `feat/thing` → PR to `dev` → merge → release: `dev → main` (tagged)

## Implementation plans

Plans live in `docs/plans/` (gitignored — local working artifacts). Use the
`plan-authoring` skill to create a new plan before starting non-trivial work.

## Privacy and security invariants

Every feature touching cookies, storage, TLS, requests, terminal output, or MCP must
respect these invariants (see spec §23):

- Remote content never reaches the terminal as raw escape sequences
- MCP tools never expose passwords, cookie values, or tokens
- Cookies are rejected by default unless the user enables them
- `<script>` elements are detected, reported, and never executed
- Web pages cannot invoke MCP tools

## AI assistant skills (`.claude/`)

| Skill | Purpose |
| ----- | ------- |
| `definition-of-done` | Review whether a change is complete before opening a PR |
| `plan-authoring` | Author an executable phased implementation plan |
| `pr-check-release` | Verify and push a branch, open a PR to `dev` |
| `pr-merge-dev` | Merge a green PR into `dev` and clean up |
| `release-check` | Review whether `dev` is ready to promote to `main` |
