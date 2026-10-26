//! Live tree filtering for Filofinder, powered by the broot library.
//!
//! This links broot (<https://dystroy.org/broot>, MIT, Denys Séguret) as a
//! crate and drives its own tree builder, so a query typed in Filofinder
//! finds, scores, ranks and prunes exactly like broot does in a terminal: the
//! same pattern syntax (fuzzy by default, `/regex/`, `^start`, `c/content`,
//! `&`, `|`, `!`), the same .gitignore handling and the same "best matches
//! first, stop when the screen is full" search budget.
//!
//! What we take from broot is the list of tree lines; everything visual is
//! done natively by the app.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use broot::{
    app::AppContext,
    command::CommandParts,
    conf::{Conf, parse_default_flags},
    pattern::InputPattern,
    task_sync::Dam,
    tree::{TreeLine, TreeLineType, TreeOptions},
    tree_build::TreeBuilder,
    verb::VerbStore,
};
use termimad::{
    TimedEvent,
    crossbeam::channel::{self, Sender},
    crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers},
};

/// What one result row points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Dir,
    File,
    /// Symlink whose target is a directory (navigable like a folder).
    SymlinkToDir,
    /// Symlink to a file, or to something we couldn't classify.
    Symlink,
    BrokenSymlink,
}

/// One line of a broot tree, minus the root line and broot's
/// "N unlisted" pruning lines.
#[derive(Debug, Clone)]
pub struct TreeRow {
    /// Path relative to the searched root, `/`-separated (e.g. `src/app/main.rs`).
    pub subpath: String,
    /// Last path component, as broot displays it.
    pub name: String,
    /// 1 for direct children of the root, 2 for grandchildren, …
    pub depth: u16,
    pub kind: RowKind,
    pub size: u64,
    pub modified_ms: i64,
    /// Inode number (0 where unavailable).
    pub file_id: u64,
    pub hidden: bool,
    /// True when this line itself matches the pattern (false for the parent
    /// folders broot keeps to show where matches live).
    pub direct_match: bool,
    /// broot's score for this line (0 without a pattern).
    pub score: i32,
    /// Char indices (not bytes) in `name` of the characters the pattern
    /// matched, for highlighting. Empty for content searches and for parents.
    pub match_pos: Vec<u32>,
    /// For folders: how many children broot left out of the tree.
    pub unlisted: u32,
}

#[derive(Debug, Clone)]
pub struct TreeResult {
    pub root: PathBuf,
    pub rows: Vec<TreeRow>,
    /// Index in `rows` of broot's best match, if any.
    pub best: Option<usize>,
    /// Number of matching entries broot pruned out of the visible tree.
    pub unlisted: u32,
    pub elapsed: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeError {
    /// `Search::cancel` was called (e.g. the user typed another key).
    Cancelled,
    /// The search ran past the caller's time budget.
    TimedOut,
    /// The query isn't a valid broot pattern (e.g. a bad regex).
    BadPattern(String),
    /// The root doesn't exist or isn't a folder, or broot failed otherwise.
    Failed(String),
}

impl std::fmt::Display for TreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "search cancelled"),
            Self::TimedOut => write!(f, "search took too long"),
            Self::BadPattern(m) => write!(f, "invalid pattern: {m}"),
            Self::Failed(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for TreeError {}

#[derive(Debug, Clone)]
pub struct TreeQuery {
    pub root: PathBuf,
    /// broot pattern, as typed in broot's input (e.g. `mod`, `/^ma.*rs$/`).
    pub pattern: String,
    /// How many lines the tree should fill, like the terminal height for
    /// broot. broot searches until it has enough good matches (or ~1 s has
    /// passed with some), then prunes to roughly this many lines.
    pub max_rows: usize,
    pub show_hidden: bool,
    pub respect_git_ignore: bool,
    /// Hard stop, on top of broot's own budget. `None` = no limit.
    pub budget: Option<Duration>,
}

impl TreeQuery {
    pub fn new(root: impl Into<PathBuf>, pattern: impl Into<String>, max_rows: usize) -> Self {
        Self {
            root: root.into(),
            pattern: pattern.into(),
            max_rows,
            show_hidden: false,
            respect_git_ignore: true,
            budget: Some(Duration::from_secs(10)),
        }
    }
}

/// broot's context: built-in default configuration (no user conf.hjson is
/// read, so results don't depend on a terminal setup) and verb store.
/// Built once per process and shared by all searches (it's read-only).
struct Ctx(AppContext);
fn context() -> Result<&'static AppContext, TreeError> {
    static CTX: OnceLock<Result<Ctx, String>> = OnceLock::new();
    CTX.get_or_init(|| {
        let mut conf = Conf::default();
        conf.true_colors = Some(false);
        let args = parse_default_flags("").map_err(|e| e.to_string())?;
        let verbs = VerbStore::new(&mut conf).map_err(|e| e.to_string())?;
        AppContext::from(args, verbs, &conf).map(Ctx).map_err(|e| e.to_string())
    })
    .as_ref()
    .map(|c| &c.0)
    .map_err(|e| TreeError::Failed(format!("broot init failed: {e}")))
}

/// A cancellable search. Create one per query, keep a clone of it where
/// keystrokes arrive and call [`Search::cancel`] when the query changes;
/// run it on a background thread with [`Search::run`].
#[derive(Clone)]
pub struct Search {
    inner: Arc<SearchInner>,
}

struct SearchInner {
    tx: Sender<TimedEvent>,
    rx: channel::Receiver<TimedEvent>,
    cancelled: AtomicBool,
}

fn wake_event() -> TimedEvent {
    TimedEvent::new(Event::Key(KeyEvent::new(KeyCode::Null, KeyModifiers::NONE)))
}

impl Default for Search {
    fn default() -> Self {
        Self::new()
    }
}

impl Search {
    pub fn new() -> Self {
        let (tx, rx) = channel::unbounded();
        Self {
            inner: Arc::new(SearchInner {
                tx,
                rx,
                cancelled: AtomicBool::new(false),
            }),
        }
    }

    /// Stop the search as soon as broot next checks (between folder levels).
    /// Safe to call from any thread, any number of times, before or during `run`.
    pub fn cancel(&self) {
        if !self.inner.cancelled.swap(true, Ordering::SeqCst) {
            let _ = self.inner.tx.send(wake_event());
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::SeqCst)
    }

    /// Run the query. Blocking: call it off the main thread.
    pub fn run(&self, q: &TreeQuery) -> Result<TreeResult, TreeError> {
        if self.is_cancelled() {
            return Err(TreeError::Cancelled);
        }
        let start = Instant::now();
        let con = context()?;
        let root = q.root.clone();
        if !root.is_dir() {
            return Err(TreeError::Failed(format!("not a folder: {}", root.display())));
        }

        let mut options = TreeOptions::default();
        options.show_hidden = q.show_hidden;
        options.respect_git_ignore = q.respect_git_ignore;
        let raw = q.pattern.trim().to_string();
        if !raw.is_empty() {
            let parts = CommandParts::from(raw.clone());
            options.pattern = InputPattern::new(raw, &parts.pattern, con)
                .map_err(|e| TreeError::BadPattern(e.to_string()))?;
        }
        let pattern = options.pattern.pattern.clone();
        let object = pattern.object();

        // Time budget: a watchdog thread wakes the dam when it runs out. It
        // exits as soon as the search ends (its `done` channel disconnects).
        let timed_out = Arc::new(AtomicBool::new(false));
        let (done_tx, done_rx) = channel::bounded::<()>(0);
        if let Some(budget) = q.budget {
            let tx = self.inner.tx.clone();
            let flag = timed_out.clone();
            thread::spawn(move || {
                if let Err(channel::RecvTimeoutError::Timeout) = done_rx.recv_timeout(budget) {
                    flag.store(true, Ordering::SeqCst);
                    let _ = tx.send(wake_event());
                }
            });
        }

        let dam = Dam::from(self.inner.rx.clone());
        let builder = TreeBuilder::from(root.clone(), options, q.max_rows.max(1), con)
            .map_err(|e| TreeError::Failed(e.to_string()))?;
        let built = builder.build_tree(false, &dam);
        drop(done_tx);
        let mut tree = match built {
            Ok(t) => t,
            Err(broot::errors::TreeBuildError::Interrupted) => {
                return Err(if self.is_cancelled() {
                    TreeError::Cancelled
                } else if timed_out.load(Ordering::SeqCst) {
                    TreeError::TimedOut
                } else {
                    TreeError::Cancelled
                });
            }
            Err(e) => return Err(TreeError::Failed(e.to_string())),
        };
        if self.is_cancelled() {
            return Err(TreeError::Cancelled);
        }

        tree.try_select_best_match();
        let best_line = tree.selection;
        let mut rows = Vec::with_capacity(tree.lines.len());
        let mut best = None;
        let mut unlisted = 0u32;
        for (i, line) in tree.lines.iter().enumerate() {
            if i == 0 {
                continue; // the root itself
            }
            if matches!(line.line_type, TreeLineType::Pruning) {
                unlisted += line.unlisted as u32;
                continue;
            }
            if i == best_line && line.direct_match {
                best = Some(rows.len());
            }
            rows.push(row_from(line, &pattern, object.subpath && !object.name));
        }
        Ok(TreeResult {
            root,
            rows,
            best,
            unlisted,
            elapsed: start.elapsed(),
        })
    }
}

/// Convenience: run a query without keeping a cancel handle.
pub fn filter_tree(q: &TreeQuery) -> Result<TreeResult, TreeError> {
    Search::new().run(q)
}

fn row_from(line: &TreeLine, pattern: &broot::pattern::Pattern, on_path: bool) -> TreeRow {
    let kind = match &line.line_type {
        TreeLineType::Dir => RowKind::Dir,
        TreeLineType::SymLink { final_is_dir: true, .. } => RowKind::SymlinkToDir,
        TreeLineType::SymLink { .. } => RowKind::Symlink,
        TreeLineType::BrokenSymLink(_) => RowKind::BrokenSymlink,
        _ => RowKind::File,
    };
    let match_pos = if line.direct_match {
        match_positions(line, pattern, on_path)
    } else {
        Vec::new()
    };
    TreeRow {
        subpath: line.subpath.clone(),
        name: line.name.clone(),
        depth: line.depth,
        kind,
        size: line.metadata.len(),
        modified_ms: modified_ms(&line.metadata),
        file_id: file_id(&line.metadata),
        hidden: line.name.starts_with('.'),
        direct_match: line.direct_match,
        score: line.score,
        match_pos,
        unlisted: line.unlisted as u32,
    }
}

/// Positions of matched chars within the leaf name. For path patterns
/// (e.g. `/src/ma`) broot matches against the subpath, so only the
/// positions falling in the last component are kept, shifted.
fn match_positions(line: &TreeLine, pattern: &broot::pattern::Pattern, on_path: bool) -> Vec<u32> {
    if on_path {
        let Some(m) = pattern.search_string(&line.subpath) else {
            return Vec::new();
        };
        let total = line.subpath.chars().count();
        let leaf = line.name.chars().count();
        let offset = total.saturating_sub(leaf);
        m.pos
            .iter()
            .filter(|&&p| p >= offset)
            .map(|&p| (p - offset) as u32)
            .collect()
    } else {
        pattern
            .search_string(&line.name)
            .map(|m| m.pos.iter().map(|&p| p as u32).collect())
            .unwrap_or_default()
    }
}

#[cfg(unix)]
fn file_id(md: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    md.ino()
}

#[cfg(not(unix))]
fn file_id(_: &std::fs::Metadata) -> u64 {
    0
}

fn modified_ms(md: &std::fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Absolute path of a row.
pub fn row_path(root: &Path, row: &TreeRow) -> PathBuf {
    root.join(&row.subpath)
}
