# Development Workflow

## Branch model

```
main          Stable releases, tagged with semver (v0.1.0, v0.2.0, …)
dev           Integration branch (all PRs target this)
feat/…        Feature branches
fix/…         Bug fix branches
doc/…         Documentation-only branches
chore/…       Tooling, CI, dependency updates
refactor/…    Refactors with no behavior change
```

Never commit directly to `main` or `dev`.

## Starting work

1. Branch from `dev`:
   ```bash
   git checkout dev
   git pull origin dev
   git checkout -b feat/my-thing
   ```

2. For non-trivial work, write down the decisions, scope, and phases before touching
   code.

3. Implement

## Quality gates

All must pass before opening a PR. Run from the repo root:

```bash
make fmt-check    # Formatting (auto-fix with: make fmt)
make lint         # Clippy (-D warnings, all targets)
make check        # Compile check
make test         # Unit tests
make test-full    # All tests, all features
```

## Opening a PR

Before pushing, run all quality gates and verify the branch merges cleanly into `dev`:

```bash
make fmt-check && make lint && make check && make test-full
git fetch origin
git merge-base HEAD origin/dev  # confirm the branch is up to date
git push -u origin feat/my-thing
gh pr create --base dev --title "feat(scope): summary" --body "..."
```

PR title must follow Conventional Commits: `type(scope): imperative summary`.

## Merging

PRs to `dev` are reviewed for ~1 day. Merge with squash:

```bash
gh pr merge <number> --squash --delete-branch
```

## Releasing

When `dev` is ready to release, a maintainer verifies CI is green and all quality gates
pass on `dev`, then merges and tags:
   ```bash
   git checkout main
   git merge --ff-only origin/dev
   git tag -s v0.1.0 -m "v0.1.0"
   git push origin main --tags
   ```

## Commit style

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(terminal): add slash command autocomplete
fix(network): handle redirect loops correctly
doc(spec): clarify cookie policy defaults
chore(deps): update ratatui to 0.29
```

Types: `feat`, `fix`, `doc`, `chore`, `refactor`, `test`, `perf`

Scopes match crate names: `core`, `network`, `html`, `css`, `layout`, `storage`,
`privacy`, `terminal`, `mcp`, `cli`. Omit the scope for cross-cutting changes.

No `Co-Authored-By` trailers, no `wip`, no placeholder messages.

## Security

Any change touching TLS, cookies, request filtering, MCP permissions, terminal output
sanitization, or URL handling requires a security review before opening the PR.
