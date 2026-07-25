# Puma

A modern, open-source, text-first browser for the terminal.

Puma is a native browser designed for reading and navigating web documents from the
command line. It renders HTML documents as readable terminal text, with no JavaScript,
no Electron, and no Chromium.

```
HTML → Semantic document → Text layout → Terminal
```

## Status

Early development. A single page renders end to end. `puma <url>` fetches one HTTP or
HTTPS page, parses it into the semantic document model, lays it out as text, and shows
it in a scrollable read-only viewport. Two `Esc` presses quit. Everything below under
[Planned features](#planned-features) is still ahead.

```
$ puma example.com   # fetch, render, and scroll the page; Esc Esc quits
$ puma               # terminal mode (default) → opens a blank page; Esc Esc quits
$ puma mcp           # MCP stdio mode          → placeholder, reports NAVIGATION_FAILED
```

## What works

- **Fetch one page** — one `http://` or `https://` URL over real TLS, following
  redirects up to a limit, with a maximum response size and lossy UTF-8 decoding
- **Text rendering** — HTML5 parsed into the semantic document model and laid out as
  terminal text: headings, bullet list items, verbatim code, indented quotes, and
  word-wrapped paragraphs; `<script>` content is suppressed and counted
- **Scrollable viewport** — arrows and PageUp/PageDown scroll; a status line shows the
  page label and scroll position; `Esc Esc` quits and `Ctrl+C` exits immediately

A blank page (`puma` with no URL) and an error page (a load that fails) both open in the
same viewport and quit the same way.

## Planned features

None of these are implemented yet. They describe where the project is headed.

- **Text-first rendering** — HTML5 parsed into a semantic document model, rendered as
  clean terminal text with proper heading hierarchy, lists, tables, and code blocks
- **Tabs** — multiple tabs with independent state per tab
- **Privacy by default** — cookies rejected by default; no localStorage, no IndexedDB;
  configurable HTTP cache; private browsing sessions with isolated temporary state
- **Unified command bar** — slash commands for navigation, search, favorites, and
  browser actions with fuzzy autocomplete
- **Find in page** — search the rendered document with match highlighting and counts
- **Favorites and history** — organize bookmarks into folders; configurable history modes
- **MCP server** — programmatic browser access via the Model Context Protocol (stdio)
- **Themeable** — data-driven themes; built-in dark, light, amber, phosphor themes
- **Unicode** — full grapheme cluster support; CJK; bidirectional text; emoji-safe layout
- **Scriptless** — `<script>` elements are detected, reported, and ignored

## Building

Requires Rust stable. Clone and build:

```bash
git clone https://github.com/meerita/puma-browser
cd puma-browser
make build
```

The release binary is written to `target/release/puma`. Run `puma example.com` to fetch
and render a page, or `puma` for a blank page; `Esc Esc` quits.

### Windows

`make` is not available by default on Windows, but it is not required: every `make`
target is a one-line wrapper around a `cargo` command, so cargo can be called directly.

```bash
cargo build --release
```

The binary is written to `target/release/puma.exe`.

Two native dependencies matter on Windows:

- **MSVC build tools** — `rusqlite` is used with the `bundled` feature, which compiles
  SQLite from C via the `cc` crate. Install the *Desktop development with C++* workload
  (or the standalone Visual Studio Build Tools) so `cl.exe` is available to `cc`.
- **TLS** — `reqwest` is configured with `rustls` and `default-features = false`, so
  OpenSSL and perl are *not* needed.

Verified on Windows 11 with Rust 1.97.1 (`x86_64-pc-windows-msvc`) and Visual Studio
2022 Build Tools: full release build of the workspace succeeds with no extra setup.

## Development

```bash
make fmt          # Fix formatting
make fmt-check    # Check formatting
make lint         # Clippy (strict)
make check        # Compile check
make test         # Run tests
```

Without `make`, the equivalent cargo commands are:

```bash
cargo fmt --all                                              # fmt
cargo fmt --all -- --check                                   # fmt-check
cargo clippy --all-targets --all-features -- -D warnings     # lint
cargo check --all-targets --all-features                     # check
cargo test --all                                             # test
```

See [docs/process/README.md](docs/process/README.md) for the full development workflow.

## Architecture

[docs/architecture/README.md](docs/architecture/README.md)

## License

MIT
