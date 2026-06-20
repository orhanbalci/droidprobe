//! Application state. Holds the current view and the most recent JSON snapshot
//! for each command id. Keeping snapshots as raw `serde_json::Value` means the
//! UI layer can render whatever commands happen to be polled without the state
//! struct needing to know every concrete output type.

use std::collections::HashMap;

use droidprobe_core::poller::PollUpdate;
use droidprobe_parser::model::{Component, PackageDetail, PackageRef};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Overview,
    Packages,
    Logs,
}

/// Which sub-pane of the package detail view is currently shown, cycled with
/// `←`/`→`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailTab {
    #[default]
    Permissions,
    Activities,
    Services,
    Receivers,
    Providers,
}

impl DetailTab {
    const ALL: [DetailTab; 5] = [
        DetailTab::Permissions,
        DetailTab::Activities,
        DetailTab::Services,
        DetailTab::Receivers,
        DetailTab::Providers,
    ];

    pub fn title(self) -> &'static str {
        match self {
            DetailTab::Permissions => "Permissions",
            DetailTab::Activities => "Activities",
            DetailTab::Services => "Services",
            DetailTab::Receivers => "Receivers",
            DetailTab::Providers => "Providers",
        }
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// The component list this tab shows, or `None` for `Permissions` (which
    /// renders [`PackageDetail::permissions`] instead).
    pub fn components(self, detail: &PackageDetail) -> Option<&[Component]> {
        match self {
            DetailTab::Permissions => None,
            DetailTab::Activities => Some(&detail.activities),
            DetailTab::Services => Some(&detail.services),
            DetailTab::Receivers => Some(&detail.receivers),
            DetailTab::Providers => Some(&detail.providers),
        }
    }
}

/// Selection and on-demand detail-fetch state for the packages view. The
/// list itself isn't stored here — it's derived from the `"package.list"`
/// snapshot each draw, since the poller already keeps that fresh.
#[derive(Debug, Default)]
pub struct PackagesState {
    pub selected: usize,
    /// Package name currently being fetched via `"package.detail"`, if any.
    pub pending: Option<String>,
    pub detail: Option<PackageDetail>,
    pub detail_error: Option<String>,
    /// Fuzzy-search query, live-filters the list as it's typed.
    pub search: String,
    /// Whether keystrokes are currently being captured into `search` rather
    /// than treated as list navigation.
    pub search_active: bool,
    /// Selected/scrolled-to row in the permissions table of the detail pane.
    pub perm_selected: usize,
    /// Which sub-pane of the detail view is active.
    pub detail_tab: DetailTab,
    /// Selected/scrolled-to row in the active component table (Activities
    /// etc). Kept separate from `perm_selected` so switching tabs doesn't
    /// disturb each other's scroll position.
    pub component_selected: usize,
}

impl PackagesState {
    pub fn move_selection(&mut self, delta: isize, len: usize) {
        if len == 0 {
            return;
        }
        let current = self.selected as isize;
        let next = (current + delta).clamp(0, len as isize - 1);
        self.selected = next as usize;
    }

    pub fn move_perm_selection(&mut self, delta: isize, len: usize) {
        if len == 0 {
            return;
        }
        let current = self.perm_selected as isize;
        let next = (current + delta).clamp(0, len as isize - 1);
        self.perm_selected = next as usize;
    }

    pub fn move_component_selection(&mut self, delta: isize, len: usize) {
        if len == 0 {
            return;
        }
        let current = self.component_selected as isize;
        let next = (current + delta).clamp(0, len as isize - 1);
        self.component_selected = next as usize;
    }

    pub fn cycle_tab(&mut self, forward: bool) {
        self.detail_tab = if forward {
            self.detail_tab.next()
        } else {
            self.detail_tab.prev()
        };
        self.component_selected = 0;
    }

    pub fn apply_detail_result(&mut self, name: &str, result: Result<PackageDetail, String>) {
        // Drop results for a package we've since navigated away from.
        if self.pending.as_deref() != Some(name) {
            return;
        }
        self.pending = None;
        self.perm_selected = 0;
        self.component_selected = 0;
        match result {
            Ok(detail) => {
                self.detail = Some(detail);
                self.detail_error = None;
            }
            Err(e) => {
                self.detail = None;
                self.detail_error = Some(e);
            }
        }
    }
}

impl View {
    pub fn title(&self) -> &'static str {
        match self {
            View::Overview => "Overview",
            View::Packages => "Packages",
            View::Logs => "Logs",
        }
    }
}

pub struct App {
    pub running: bool,
    pub view: View,
    /// Latest output per command id, plus any error string.
    pub snapshots: HashMap<String, Result<Value, String>>,
    pub packages: PackagesState,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            view: View::Overview,
            snapshots: HashMap::new(),
            packages: PackagesState::default(),
        }
    }

    /// The current package list, decoded from the `"package.list"` snapshot.
    pub fn package_list(&self) -> Vec<PackageRef> {
        self.snapshot_ok("package.list")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }

    /// The package list narrowed by `packages.search`, best matches first.
    /// Returns the full list, in original order, when the query is empty.
    pub fn filtered_packages(&self) -> Vec<PackageRef> {
        let all = self.package_list();
        if self.packages.search.is_empty() {
            return all;
        }
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(i64, PackageRef)> = all
            .into_iter()
            .filter_map(|p| {
                matcher
                    .fuzzy_match(&p.name, &self.packages.search)
                    .map(|score| (score, p))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, p)| p).collect()
    }

    pub fn next_view(&mut self) {
        self.view = match self.view {
            View::Overview => View::Packages,
            View::Packages => View::Logs,
            View::Logs => View::Overview,
        };
    }

    pub fn apply_update(&mut self, update: PollUpdate) {
        self.snapshots.insert(update.command_id, update.result);
    }

    /// Convenience accessor for a successfully-fetched snapshot.
    pub fn snapshot_ok(&self, id: &str) -> Option<&Value> {
        self.snapshots.get(id).and_then(|r| r.as_ref().ok())
    }

    /// The error message for `id`, if its last poll failed. Distinguishing
    /// this from "not polled yet" (`None`/no entry) is what lets the UI show
    /// a real error instead of a permanently spinning "fetching…".
    pub fn snapshot_err(&self, id: &str) -> Option<&str> {
        self.snapshots.get(id)?.as_ref().err().map(String::as_str)
    }
}
