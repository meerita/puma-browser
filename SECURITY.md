# Security Policy

Puma treats all remote content as untrusted. Security reports are taken seriously and
handled privately until a fix is available.

## Supported versions

Puma is in early development on the `0.x` line. Security fixes are applied to the latest
release and to the `dev` integration branch. Older `0.x` versions are not maintained
separately: upgrade to the latest release to receive fixes.

| Version | Supported |
| ------- | --------- |
| Latest `0.x` release | Yes |
| `dev` branch | Yes |
| Older versions | No |

## Reporting a vulnerability

Do not open a public issue for a security vulnerability.

Report it privately in one of two ways:

- Use GitHub's [private vulnerability reporting](https://github.com/meerita/puma-browser/security/advisories/new)
  on this repository.
- Or email the maintainer at meerita@icloud.com.

Please include enough detail to reproduce the issue:

- The affected version or commit.
- The URL, HTML input, MCP request, or configuration that triggers the problem.
- What you expected to happen and what actually happened.
- Your platform and Rust version.

You will receive an acknowledgment within a few days. Once the issue is confirmed, a
fix and a coordinated disclosure timeline will be agreed with you before any public
announcement.

## Scope

Puma's security model prevents web pages from reaching the terminal as raw bytes,
leaking secrets, or invoking MCP tools. Reports in the following areas are in scope:

- **Terminal safety.** Remote content that reaches the terminal as raw escape sequences,
  including ANSI injection, terminal title injection, clipboard (OSC 52) injection, or
  any other operating-system command sequence sourced from a web page.
- **Secret exposure.** Cookie values, passwords, authentication tokens, or proxy
  credentials appearing in logs, `Debug` output, error messages, or MCP responses.
- **MCP isolation.** A web page that can invoke MCP tools, discover MCP clients, change
  privacy settings, or reach `file://` or private-network resources through the server.
- **URL and request handling.** Scheme validation bypass, HTTPS to HTTP downgrade on
  redirect, redirects to `file://` from a web origin, or SSRF past the guard on loopback
  and private address ranges.
- **Resource limits.** Missing or bypassable limits on response size, DOM depth, node
  count, redirect count, or table dimensions that allow denial of service.

## Out of scope

- Vulnerabilities in third-party dependencies that already have a public advisory. Those
  are tracked through `make audit` and dependency updates.
- Attacks that require a modified build of Puma or local machine access.
- JavaScript execution: Puma has no JavaScript runtime. `<script>` elements are detected,
  counted, and never executed.
