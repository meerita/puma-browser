<div align="center">

# Puma, The Browser

**A native, text-first web browser for the terminal.**

Puma renders web documents as readable terminal text. No JavaScript, no Electron,
no Chromium. One native binary on Linux, macOS, and Windows.

[![CI](https://github.com/meerita/puma-browser/actions/workflows/ci.yml/badge.svg)](https://github.com/meerita/puma-browser/actions/workflows/ci.yml)
[![Security audit](https://github.com/meerita/puma-browser/actions/workflows/security-audit.yml/badge.svg)](https://github.com/meerita/puma-browser/actions/workflows/security-audit.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-000000?logo=rust)](https://www.rust-lang.org)
[![GitHub stars](https://img.shields.io/github/stars/meerita/puma-browser?style=flat&logo=github)](https://github.com/meerita/puma-browser/stargazers)

</div>

```
HTML → Semantic document → Text layout → Terminal or MCP
```

Puma reads and navigates the web from the command line. It fetches a page over
HTTP or HTTPS, parses the HTML into a semantic document model, lays it out as text,
and renders it in a scrollable terminal viewport. The same core serves a built-in
MCP server, so agents can read the web without a browser engine.

## Why Puma

- **No browser engine.** No JavaScript runtime, no DOM scripting, no Chromium, no
  Electron. Puma parses HTML into a semantic tree and renders text from it.
- **Native and small.** A single Rust binary with a release profile tuned for size
  and speed: link-time optimization, a single codegen unit, and a stripped binary.
  Startup is immediate; there is no engine to boot.
- **Private by default.** Cookies are rejected unless the user enables them. There is
  no localStorage, sessionStorage, IndexedDB, or Service Worker storage.
- **Terminal-safe.** Remote content never reaches the terminal as raw bytes. Every
  page passes through the layout engine into an off-screen cell buffer, and the
  renderer writes escape sequences from that buffer, not from the source document.
- **Agent-ready.** A built-in MCP server exposes read-only tools (`browser_open`,
  `browser_read`, `browser_list_links`) over stdio. Web content is always tagged
  untrusted, and web pages can never invoke MCP tools.

## Status

Active development. `puma <url>` fetches one HTTP or HTTPS page, parses it into a full
semantic document tree with a real CSS cascade, and renders it as scrollable terminal
text. The browser persists history with URL autocomplete, enforces a first-party and
third-party cookie policy, and exposes a full-screen settings panel. The
[roadmap](#roadmap) lists what is still ahead.

```
$ puma example.com   # fetch, render, and scroll the page; Esc Esc quits
$ puma               # terminal mode (default): opens a blank page; Esc Esc quits
$ puma mcp           # MCP stdio server
```

### What works today

- **Document pipeline.** HTML5 parsed into a semantic tree with a real CSS cascade:
  headings, nested lists, tables, inline styled runs, code blocks, quotes, `<hr>`, and
  `<q>` citation links with hover-previewable `cite` targets. `<script>` content is
  suppressed and counted, never executed.
- **Persistent history.** SQLite-backed visit history with URL autocomplete, search,
  and per-site or full clearing, viewable through `/history`.
- **Cookie policy.** First-party and third-party classification by registrable domain,
  an `allow`/`session`/`ask`/`reject` policy (default `reject`), a session jar, and
  `/cookies` inspection with persisted per-site exceptions.
- **Network hardening.** HTTP/2 and zstd/gzip/brotli decompression, a 32 KiB response
  header size cap enforced on every redirect hop, and an honest outbound request
  identity (`User-Agent`, `Accept-Language`, `DNT`, `Sec-GPC`) built from the app
  version and detected OS/arch/locale.
- **Settings panel.** `/settings` (alias `/config`) is a full-screen view over a real
  SQLite-backed config store: toggles and radios apply instantly, text inputs auto-save
  after a short idle.
- **Unified command bar.** A slash-command palette with fuzzy autocomplete for
  `/open`, `/search`, `/reload`, `/back`, `/history`, `/cookies`, `/settings`, and
  `/help`. `/search` runs a no-JavaScript web search.
- **Link navigation and copy-on-select.** Tab cycles link focus and keeps the focused
  link in view, click and Enter follow it, Backspace goes back, and anchors jump within
  the page. Mouse-driven text selection copies to the clipboard with visual feedback.
- **Scrollable viewport.** Arrows and PageUp/PageDown scroll. A status line shows the
  page label and scroll position. `Esc Esc` quits and `Ctrl+C` exits immediately.
- **MCP server.** Read-only stdio tools for opening a URL, reading its text, and
  listing its links, with an SSRF guard on loopback and private address ranges.

A blank page (`puma` with no URL) and an error page (a load that fails) both open in the
same viewport and quit the same way.

## Roadmap

None of these are implemented yet. They describe where the project is headed.

- **Tabs.** Multiple tabs with independent state per tab.
- **Bookmarks, forms, and downloads.** Favorites organized into folders, form
  submission, and streamed downloads.
- **`<dialog>` element support.** Scriptless dialogs rendered as a non-modal
  auto-popover.
- **Find in page.** Search the rendered document with match highlighting and counts.
- **Themeable.** Data-driven themes: dark, light, amber, phosphor.
- **Full Unicode.** Bidirectional text reordering and CJK line-breaking without
  spaces.
- **Privacy dashboard and private browsing.** Cookie, storage, and cache summary with
  an isolated temporary session mode.
- **Proxy support.** HTTP, HTTPS, and SOCKS proxying, the last remaining item in the
  networking milestone.

## Install

### From source

Puma requires Rust stable.

```bash
git clone https://github.com/meerita/puma-browser
cd puma-browser
make build
```

The release binary is written to `target/release/puma`. Run `puma example.com` to fetch
and render a page, or `puma` for a blank page. `Esc Esc` quits.

### Windows

`make` is not required on Windows. Every `make` target wraps a single `cargo` command,
so call cargo directly:

```bash
cargo build --release
```

Cargo writes the binary to `target/release/puma.exe`. Two native dependencies matter:

- **MSVC build tools.** `rusqlite` uses the `bundled` feature, which compiles SQLite
  from C through the `cc` crate. Install the "Desktop development with C++" workload (or
  the standalone Visual Studio Build Tools) so `cl.exe` is available to `cc`.
- **TLS.** `reqwest` uses `rustls` with `default-features = false`, so OpenSSL and perl
  are not needed.

The full release build succeeds on Windows 11 with Rust 1.97.1
(`x86_64-pc-windows-msvc`) and Visual Studio 2022 Build Tools, with no extra setup.

## Development

```bash
make fmt          # Fix formatting
make fmt-check    # Check formatting
make lint         # Clippy (strict, -D warnings)
make check        # Compile check
make test         # Run tests
```

Without `make`, run the equivalent cargo commands:

```bash
cargo fmt --all                                              # fmt
cargo fmt --all -- --check                                   # fmt-check
cargo clippy --all-targets --all-features -- -D warnings     # lint
cargo check --all-targets --all-features                     # check
cargo test --all                                             # test
```

See [docs/process/README.md](docs/process/README.md) for the full development workflow.

## Documentation

- [Documentation index](docs/README.md)
- [Architecture](docs/architecture/README.md)
- [Architecture overview](docs/architecture/overview.md)
- [Network layer](docs/architecture/network.md)
- [Development workflow](docs/process/README.md)

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) for the branch model,
commit style, quality gates, and how to open a pull request. All participants are
expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Puma is released under the [MIT License](LICENSE).
