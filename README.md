# Puma

A modern, open-source, text-first browser for the terminal.

Puma is a native browser designed for reading and navigating web documents from the
command line. It renders HTML documents as readable terminal text, supports tabs,
bookmarks, forms, and downloads.

**No JavaScript. No Electron. No Chromium.**

```
HTML → Semantic document → Text layout → Terminal
```

## Features

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

## Status

Early development.

## Building

Requires Rust stable (1.80+). Clone and build:

```bash
git clone https://github.com/your-org/puma-browser
cd puma-browser
make build
```

The binary is at `target/release/puma`.

## Development

```bash
make fmt          # Fix formatting
make fmt-check    # Check formatting
make lint         # Clippy (strict)
make check        # Compile check
make test         # Run tests
```

See [docs/process/README.md](docs/process/README.md) for the full development workflow.

## Architecture

[docs/architecture/README.md](docs/architecture/README.md)

## License

Apache-2.0 OR MIT
