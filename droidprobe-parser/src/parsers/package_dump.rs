//! Parser for `adb shell dumpsys package <name>` output.
//!
//! `dumpsys package` is notoriously version-dependent across AOSP forks, so
//! this parser only extracts fields verified against real device output
//! (Android 14/15, Samsung build): version info, the
//! requested/install/runtime permission blocks, and components via the
//! Activity/Receiver/Service/Provider Resolver Table sections.
//!
//! Resolver tables only list components that declare an `<intent-filter>`,
//! so anything invoked purely by explicit `ComponentName` (most services) or
//! by authority URI (all content providers) won't appear here — that's a
//! real gap in this data source, not a parsing bug. See
//! [`crate::model::Component`] for why there's no `exported` field either.

use std::collections::{HashMap, HashSet};

use crate::model::{Component, PackageDetail, Permission, ProtectionLevel};
use crate::{Parse, ParseResult};

pub struct PackageDumpParser;

#[derive(Default, Clone, Copy, PartialEq)]
enum PermSection {
    #[default]
    None,
    Requested,
    Install,
    Runtime,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Table {
    Activity,
    Receiver,
    Service,
    Provider,
}

#[derive(Default)]
struct ComponentAccum {
    actions: Vec<String>,
    categories: HashSet<String>,
    permission: Option<String>,
}

/// Headers that end a resolver-table region. `dumpsys package`'s real-world
/// indentation is inconsistent (we've seen the same header indented in one
/// dump and flush-left in another), so detection is by header text, not
/// column position.
const TABLE_ENDING_HEADERS: &[&str] = &[
    "Domain verification status:",
    "Permissions:",
    "Key Set Manager:",
    "Packages:",
    "Queries:",
    "Dexopt state:",
    "Compiler stats:",
    "Historical install Logging info",
    "HeimdAllFS state:",
];

impl PackageDumpParser {
    /// Parse a dump for a known package name, used by [`crate::Parse::parse`]
    /// and directly by callers who already know which package they asked for.
    pub fn parse_for(pkg: &str, raw: &str) -> ParseResult<PackageDetail> {
        let mut detail = PackageDetail {
            name: pkg.to_string(),
            ..Default::default()
        };
        let mut permissions: HashMap<String, Permission> = HashMap::new();
        let mut perm_section = PermSection::None;

        let mut current_table: Option<Table> = None;
        let mut current_component: Option<(Table, String)> = None;
        let mut components: HashMap<(Table, String), ComponentAccum> = HashMap::new();

        // Some real dumps contain a stray U+FFFD where a non-UTF8 byte got
        // lossily transcoded somewhere upstream; strip it so it doesn't
        // split tokens that should be contiguous (e.g. `pkg/Name`).
        let cleaned = raw.replace('\u{FFFD}', "");

        for line in cleaned.lines() {
            let trimmed = line.trim();

            if let Some(rest) = trimmed.strip_prefix("versionName=") {
                detail.version_name = rest.to_string();
                continue;
            }
            if trimmed.starts_with("versionCode=") {
                for tok in trimmed.split_whitespace() {
                    if let Some(v) = tok.strip_prefix("versionCode=") {
                        detail.version_code = v.parse().unwrap_or(0);
                    } else if let Some(v) = strip_any(tok, &["minSdkVersion=", "minSdk="]) {
                        detail.min_sdk = v.parse().unwrap_or(0);
                    } else if let Some(v) = strip_any(tok, &["targetSdkVersion=", "targetSdk="]) {
                        detail.target_sdk = v.parse().unwrap_or(0);
                    }
                }
                continue;
            }

            if trimmed.contains("Activity Resolver Table:") {
                current_table = Some(Table::Activity);
                current_component = None;
                continue;
            }
            if trimmed.contains("Receiver Resolver Table:") {
                current_table = Some(Table::Receiver);
                current_component = None;
                continue;
            }
            if trimmed.contains("Service Resolver Table:") {
                current_table = Some(Table::Service);
                current_component = None;
                continue;
            }
            if trimmed.contains("Provider Resolver Table:") {
                current_table = Some(Table::Provider);
                current_component = None;
                continue;
            }
            if TABLE_ENDING_HEADERS.iter().any(|h| trimmed.starts_with(h)) {
                current_table = None;
                current_component = None;
                // Fall through: "Permissions:" itself carries no permission
                // data, and the others are handled by the existing
                // perm_section state machine below (which no-ops on them).
            }

            if let Some(table) = current_table {
                if let Some((name, permission)) = parse_component_line(trimmed) {
                    let key = (table, name.clone());
                    let accum = components.entry(key.clone()).or_default();
                    if permission.is_some() {
                        accum.permission = permission;
                    }
                    current_component = Some(key);
                    continue;
                }
                if let Some(action) = parse_quoted_value(trimmed, "Action:") {
                    if let Some(key) = &current_component {
                        let accum = components.entry(key.clone()).or_default();
                        if !accum.actions.contains(&action) {
                            accum.actions.push(action);
                        }
                    }
                    continue;
                }
                if let Some(category) = parse_quoted_value(trimmed, "Category:") {
                    if let Some(key) = &current_component {
                        components
                            .entry(key.clone())
                            .or_default()
                            .categories
                            .insert(category);
                    }
                    continue;
                }
                // Sub-group headers (`Non-Data Actions:`, `Schemes:`, action
                // keys like `android.intent.action.MAIN:`) and continuation
                // lines (`mPriority=...`) carry nothing we need; skip them.
                continue;
            }

            match trimmed {
                "requested permissions:" => {
                    perm_section = PermSection::Requested;
                    continue;
                }
                "install permissions:" => {
                    perm_section = PermSection::Install;
                    continue;
                }
                "runtime permissions:" => {
                    perm_section = PermSection::Runtime;
                    continue;
                }
                _ => {}
            }

            perm_section = match perm_section {
                PermSection::Requested => match parse_permission_name(trimmed) {
                    Some(name) => {
                        permissions.entry(name.clone()).or_insert(Permission {
                            name,
                            granted: false,
                            protection_level: ProtectionLevel::Unknown,
                        });
                        perm_section
                    }
                    None => PermSection::None,
                },
                PermSection::Install => match parse_permission_grant(trimmed) {
                    Some((name, granted)) => {
                        permissions.insert(
                            name.clone(),
                            Permission {
                                name,
                                granted,
                                protection_level: ProtectionLevel::Normal,
                            },
                        );
                        perm_section
                    }
                    None => PermSection::None,
                },
                PermSection::Runtime => match parse_permission_grant(trimmed) {
                    Some((name, granted)) => {
                        permissions.insert(
                            name.clone(),
                            Permission {
                                name,
                                granted,
                                protection_level: ProtectionLevel::Dangerous,
                            },
                        );
                        perm_section
                    }
                    None => PermSection::None,
                },
                PermSection::None => PermSection::None,
            };
        }

        detail.permissions = permissions.into_values().collect();
        detail.permissions.sort_by(|a, b| a.name.cmp(&b.name));

        let mut by_table: HashMap<Table, Vec<Component>> = HashMap::new();
        for ((table, name), accum) in components {
            let mut actions = accum.actions;
            actions.sort();
            if table == Table::Activity
                && accum
                    .categories
                    .contains("android.intent.category.LAUNCHER")
                && actions.contains(&"android.intent.action.MAIN".to_string())
                && detail.launcher_activity.is_none()
            {
                detail.launcher_activity = Some(name.clone());
            }
            by_table.entry(table).or_default().push(Component {
                name,
                intent_actions: actions,
                permission: accum.permission,
            });
        }
        for components in by_table.values_mut() {
            components.sort_by(|a, b| a.name.cmp(&b.name));
        }
        detail.activities = by_table.remove(&Table::Activity).unwrap_or_default();
        detail.services = by_table.remove(&Table::Service).unwrap_or_default();
        detail.receivers = by_table.remove(&Table::Receiver).unwrap_or_default();
        detail.providers = by_table.remove(&Table::Provider).unwrap_or_default();

        detail.raw = raw.to_string();
        Ok(detail)
    }
}

impl Parse for PackageDumpParser {
    type Output = PackageDetail;

    /// Extracts the package name from the `Package [<name>] (...)` header
    /// line, then delegates to [`Self::parse_for`].
    fn parse(raw: &str) -> ParseResult<PackageDetail> {
        let name = raw
            .lines()
            .find_map(|l| {
                let rest = l.trim().strip_prefix("Package [")?;
                let (name, _) = rest.split_once(']')?;
                Some(name.to_string())
            })
            .unwrap_or_default();
        Self::parse_for(&name, raw)
    }
}

fn strip_any<'a>(tok: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|p| tok.strip_prefix(p))
}

/// A bare permission name line inside a `requested permissions:` block, e.g.
/// `android.permission.CAMERA`. Any line with `:`/`=` or whitespace isn't one.
fn parse_permission_name(line: &str) -> Option<String> {
    if line.is_empty() || line.contains(':') || line.contains('=') || line.contains(' ') {
        return None;
    }
    if !line.contains('.') {
        return None;
    }
    Some(line.to_string())
}

/// A `<permission name>: granted=<bool>, ...` line inside an `install
/// permissions:` or `runtime permissions:` block.
fn parse_permission_grant(line: &str) -> Option<(String, bool)> {
    let (name, rest) = line.split_once(':')?;
    let name = name.trim();
    if name.is_empty() || name.contains(' ') || !name.contains('.') {
        return None;
    }
    let granted = rest.contains("granted=true");
    Some((name.to_string(), granted))
}

/// A resolver-table component line, e.g.
/// `8d716df com.example.app/.BootReceiver filter c06df2c permission android.permission.X`.
/// Returns the component name (everything after the `pkg/`) and an optional
/// required permission.
fn parse_component_line(line: &str) -> Option<(String, Option<String>)> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let slash_idx = tokens.iter().position(|t| t.contains('/'))?;
    let filter_idx = tokens.iter().position(|t| *t == "filter")?;
    if filter_idx <= slash_idx {
        return None;
    }

    let tok = tokens[slash_idx];
    let after = tok.split_once('/').map(|(_, b)| b).unwrap_or("");
    // Some real dumps have a corrupted byte right after the slash that
    // lossily turns into extra whitespace, splitting the component name
    // into its own token; fall back to the next token when that happens.
    let name = if after.is_empty() {
        tokens.get(slash_idx + 1)?.to_string()
    } else {
        after.to_string()
    };

    let permission = tokens
        .iter()
        .position(|t| *t == "permission")
        .and_then(|i| tokens.get(i + 1))
        .map(|s| s.to_string());

    Some((name, permission))
}

/// Extracts the quoted value from a line like `Action: "android.intent.action.MAIN"`.
fn parse_quoted_value(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?.trim();
    let inner = rest.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.to_string())
}
