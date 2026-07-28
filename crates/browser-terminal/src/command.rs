// @file crates/browser-terminal/src/command.rs
// @description Slash-command registry, ranked palette matcher, and cookies subcommand parser.
// @layer terminal
// @created meerita <meerita@icloud.com>

/// Stable identity of a palette command. Dispatch matches on this to select a handler;
/// it carries no behavior of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandKind {
    Open,
    Search,
    Reload,
    Back,
    History,
    Cookies,
    Help,
    Quit,
    Settings,
}

/// A single command definition. The `name` and every alias are stored without the leading
/// `/`; the palette adds it when rendering. `takes_argument` drives Tab completion: a
/// command that takes an argument completes with a trailing space so the user can type it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CommandSpec {
    pub(crate) name: &'static str,
    pub(crate) aliases: &'static [&'static str],
    pub(crate) description: &'static str,
    pub(crate) takes_argument: bool,
    pub(crate) kind: CommandKind,
}

/// How well a command matched a query. Prefix matches always rank above subsequence
/// matches in the filtered palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MatchRank {
    Prefix,
    Subsequence,
}

/// A command that survived filtering, paired with the rank it matched at.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CommandMatch {
    pub(crate) spec: &'static CommandSpec,
    pub(crate) rank: MatchRank,
}

const REGISTRY: &[CommandSpec] = &[
    CommandSpec {
        name: "open",
        aliases: &[],
        description: "open a URL in the current tab",
        takes_argument: true,
        kind: CommandKind::Open,
    },
    CommandSpec {
        name: "search",
        aliases: &[],
        description: "search the web for a query",
        takes_argument: true,
        kind: CommandKind::Search,
    },
    CommandSpec {
        name: "reload",
        aliases: &[],
        description: "reload the current page",
        takes_argument: false,
        kind: CommandKind::Reload,
    },
    CommandSpec {
        name: "back",
        aliases: &[],
        description: "go back to the previous page",
        takes_argument: false,
        kind: CommandKind::Back,
    },
    CommandSpec {
        name: "history",
        aliases: &[],
        description: "list, search, or clear browsing history",
        takes_argument: true,
        kind: CommandKind::History,
    },
    CommandSpec {
        name: "cookies",
        aliases: &[],
        description: "inspect and control cookies",
        takes_argument: true,
        kind: CommandKind::Cookies,
    },
    CommandSpec {
        name: "help",
        aliases: &[],
        description: "list the available commands",
        takes_argument: false,
        kind: CommandKind::Help,
    },
    CommandSpec {
        name: "quit",
        aliases: &[],
        description: "exit the browser",
        takes_argument: false,
        kind: CommandKind::Quit,
    },
    CommandSpec {
        name: "settings",
        aliases: &["config"],
        description: "open browser settings (coming soon)",
        takes_argument: false,
        kind: CommandKind::Settings,
    },
];

/// Every registered command, in registry order.
pub(crate) fn registry() -> &'static [CommandSpec] {
    REGISTRY
}

/// Split a command-bar buffer into its command token and argument remainder. The leading
/// `/` is stripped, the token is the run of non-space characters that follows, and the
/// remainder is everything after the first whitespace, trimmed. A buffer of just `/`
/// yields an empty token and empty remainder; the caller treats an empty token as "no
/// command entered" rather than an error.
pub(crate) fn parse_command_input(buffer: &str) -> (&str, &str) {
    let trimmed = buffer.trim();
    let without_prefix = trimmed.strip_prefix('/').unwrap_or(trimmed);
    match without_prefix.split_once(char::is_whitespace) {
        Some((token, remainder)) => (token, remainder.trim()),
        None => (without_prefix, ""),
    }
}

/// Resolve a bare command token (no leading `/`) to its spec by exact match on the
/// canonical name or any alias. Case-sensitive.
pub(crate) fn resolve(token: &str) -> Option<&'static CommandSpec> {
    REGISTRY
        .iter()
        .find(|spec| spec.name == token || spec.aliases.contains(&token))
}

/// Filter the registry against a query (the text after `/`, possibly empty), returning
/// ranked matches. Prefix matches come first in registry order, then subsequence matches
/// in registry order. Each command appears at most once, taking its best rank.
pub(crate) fn filter(query: &str) -> Vec<CommandMatch> {
    if query.is_empty() {
        return REGISTRY
            .iter()
            .map(|spec| CommandMatch {
                spec,
                rank: MatchRank::Prefix,
            })
            .collect();
    }

    let ranked: Vec<Option<MatchRank>> =
        REGISTRY.iter().map(|spec| rank_spec(spec, query)).collect();

    let mut matches: Vec<CommandMatch> = collect_by_rank(&ranked, MatchRank::Prefix);
    matches.extend(collect_by_rank(&ranked, MatchRank::Subsequence));
    matches
}

/// Best rank of a single spec against the query across its name and aliases.
fn rank_spec(spec: &CommandSpec, query: &str) -> Option<MatchRank> {
    let candidates = std::iter::once(spec.name).chain(spec.aliases.iter().copied());
    let mut best: Option<MatchRank> = None;
    for candidate in candidates {
        if candidate.starts_with(query) {
            return Some(MatchRank::Prefix);
        }
        if is_subsequence(query, candidate) {
            best = Some(MatchRank::Subsequence);
        }
    }
    best
}

/// Gather commands whose best rank equals `wanted`, preserving registry order.
fn collect_by_rank(ranked: &[Option<MatchRank>], wanted: MatchRank) -> Vec<CommandMatch> {
    REGISTRY
        .iter()
        .zip(ranked)
        .filter_map(|(spec, rank)| match rank {
            Some(rank) if *rank == wanted => Some(CommandMatch { spec, rank: *rank }),
            _ => None,
        })
        .collect()
}

/// A parsed `/cookies` request. `AllowSession` and `Reject` carry the site the exception
/// applies to; `Usage` marks an argument the parser did not recognize so the caller can
/// show a usage message instead of acting on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CookiesRequest {
    Summary,
    Accepted,
    Rejected,
    Clear,
    AllowSession(String),
    Reject(String),
    Usage,
}

/// Parses a `/cookies` argument into a request. An empty argument is the summary; the
/// `accepted`, `rejected`, and `clear` subcommands take no site; `allow-session` and
/// `reject` each require a site and yield `Usage` without one. Any other subcommand is
/// `Usage`, so an unknown word surfaces a usage message rather than a silent no-op.
pub(crate) fn parse_cookies_request(remainder: &str) -> CookiesRequest {
    let trimmed = remainder.trim();
    if trimmed.is_empty() {
        return CookiesRequest::Summary;
    }
    let (subcommand, site) = match trimmed.split_once(char::is_whitespace) {
        Some((subcommand, rest)) => (subcommand, rest.trim()),
        None => (trimmed, ""),
    };
    match subcommand {
        "accepted" => CookiesRequest::Accepted,
        "rejected" => CookiesRequest::Rejected,
        "clear" => CookiesRequest::Clear,
        "allow-session" if !site.is_empty() => CookiesRequest::AllowSession(site.to_string()),
        "reject" if !site.is_empty() => CookiesRequest::Reject(site.to_string()),
        _ => CookiesRequest::Usage,
    }
}

/// True when every character of `query` appears in `candidate` in order, not necessarily
/// adjacent.
fn is_subsequence(query: &str, candidate: &str) -> bool {
    let mut candidate_chars = candidate.chars();
    query
        .chars()
        .all(|needle| candidate_chars.any(|current| current == needle))
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
