# Puma Documentation

## Contents

| Document | Description |
| -------- | ----------- |
| [architecture/README.md](architecture/README.md) | Crate map, rendering pipeline, and key design decisions |
| [architecture/overview.md](architecture/overview.md) | The two structural patterns and how they meet at `browser-core` |
| [process/README.md](process/README.md) | Branch model, quality gates, commit style, and release flow |

## Project layout

The browser is a Rust workspace. Each crate under `crates/` owns one responsibility, and
dependencies point inward: outer crates depend on inner crates, never the reverse.

```
crates/
  browser-core/       Navigation controller, tabs, history, bookmarks, forms, downloads
  browser-network/    HTTP client, TLS, redirects, proxy, cookies, cache, request filtering
  browser-html/       HTML5 parser (html5ever), semantic document builder
  browser-css/        Reduced CSS cascade for text rendering (cssparser)
  browser-layout/     Text layout engine, terminal cell renderer, word wrapping
  browser-storage/    SQLite persistence: config, profiles, history, bookmarks, themes
  browser-privacy/    Cookie policy, privacy dashboard, session isolation
  browser-terminal/   Ratatui TUI, command system, theme engine, input handling
  browser-mcp/        MCP server (stdio), tools, resources, permissions
  browser-cli/        Binary entry point
```

## Contributing to the docs

Documentation is written for engineers. Use active voice and the present tense, name the
actual crate or type, and describe behavior directly. Diagrams use Mermaid rather than
ASCII art. See [process/README.md](process/README.md) for the workflow that applies to
documentation changes.
