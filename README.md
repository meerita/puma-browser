# Puma

A modern, open-source, text-first browser for the terminal.

Puma is a native browser designed for reading and navigating web documents from the
command line. It renders HTML documents as readable terminal text, with no JavaScript,
no Electron, and no Chromium.

```
HTML → Semantic document → Text layout → Terminal
```

## Status

Early development. The project is scaffolding: the workspace compiles and the crate
foundations are in place (error taxonomies, domain identifiers, the composition root
that wires the core to its adapters), but there is no working browser yet.

Running the binary today reports a placeholder error and exits. Nothing is rendered,
fetched, or parsed end to end. The features below describe the target product, not the
current behavior.

```
$ puma          # terminal mode (default) → reports "Could not render the page"
$ puma mcp      # MCP stdio mode          → reports NAVIGATION_FAILED
```

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

The release binary is written to `target/release/puma`. It builds and runs, but only
reports the placeholder status described under [Status](#status).

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

MIT
