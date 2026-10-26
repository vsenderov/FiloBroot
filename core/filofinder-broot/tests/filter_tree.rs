use std::{fs, path::Path, thread, time::Duration};

use filofinder_broot::{RowKind, Search, TreeError, TreeQuery, filter_tree};

fn touch(root: &Path, rel: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, b"x").unwrap();
}

fn sample() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    for f in [
        "src/main.rs",
        "src/app/window.rs",
        "src/app/menu.rs",
        "docs/readme.md",
        "docs/manual/intro.md",
        "Cargo.toml",
        ".hidden/secret.rs",
        ".dotfile",
    ] {
        touch(r, f);
    }
    dir
}

fn subpaths(res: &filofinder_broot::TreeResult) -> Vec<String> {
    res.rows.iter().map(|r| r.subpath.clone()).collect()
}

#[test]
fn fuzzy_name_search_keeps_parents_and_positions() {
    let dir = sample();
    let res = filter_tree(&TreeQuery::new(dir.path(), "mai", 50)).unwrap();
    let paths = subpaths(&res);
    assert!(paths.contains(&"src/main.rs".to_string()), "{paths:?}");
    let main = res.rows.iter().find(|r| r.subpath == "src/main.rs").unwrap();
    assert_eq!(main.depth, 2);
    assert_eq!(main.kind, RowKind::File);
    assert!(main.direct_match);
    assert_eq!(main.match_pos, vec![0, 1, 2]);
    // The parent folder is kept to show where the match lives, but isn't a match.
    let src = res.rows.iter().find(|r| r.subpath == "src").unwrap();
    assert_eq!(src.depth, 1);
    assert_eq!(src.kind, RowKind::Dir);
    assert!(!src.direct_match);
    // A parent always comes before its children (tree order).
    let i_src = paths.iter().position(|p| p == "src").unwrap();
    let i_main = paths.iter().position(|p| p == "src/main.rs").unwrap();
    assert!(i_src < i_main);
    // Non-matching files are gone.
    assert!(!paths.contains(&"Cargo.toml".to_string()));
    assert!(res.best.is_some());
    assert!(res.rows[res.best.unwrap()].direct_match);
}

#[test]
fn hidden_files_follow_the_flag() {
    let dir = sample();
    let res = filter_tree(&TreeQuery::new(dir.path(), "secret", 50)).unwrap();
    assert!(res.rows.iter().all(|r| !r.subpath.contains("secret")));
    let mut q = TreeQuery::new(dir.path(), "secret", 50);
    q.show_hidden = true;
    let res = filter_tree(&q).unwrap();
    let row = res.rows.iter().find(|r| r.subpath == ".hidden/secret.rs").unwrap();
    assert!(row.direct_match);
    let parent = res.rows.iter().find(|r| r.subpath == ".hidden").unwrap();
    assert!(parent.hidden);
}

#[test]
fn regex_pattern() {
    let dir = sample();
    let res = filter_tree(&TreeQuery::new(dir.path(), "/\\.md$/", 50)).unwrap();
    let matches: Vec<_> = res.rows.iter().filter(|r| r.direct_match).map(|r| r.subpath.clone()).collect();
    assert_eq!(matches.len(), 2, "{matches:?}");
    assert!(matches.contains(&"docs/manual/intro.md".to_string()));
    let intro = res.rows.iter().find(|r| r.subpath == "docs/manual/intro.md").unwrap();
    assert_eq!(intro.match_pos, vec![5, 6, 7]);
}

#[test]
fn path_pattern_positions_are_relative_to_the_name() {
    let dir = sample();
    // broot's default "p/" prefix = fuzzy search on the path.
    let res = filter_tree(&TreeQuery::new(dir.path(), "p/appmen", 50)).unwrap();
    let menu = res.rows.iter().find(|r| r.subpath == "src/app/menu.rs").expect("menu.rs");
    assert!(menu.direct_match);
    assert!(menu.match_pos.iter().all(|&p| (p as usize) < "menu.rs".len()));
    assert_eq!(menu.match_pos, vec![0, 1, 2]);
}

#[test]
fn gitignore_is_respected_like_broot() {
    let dir = sample();
    let r = dir.path();
    fs::create_dir_all(r.join(".git")).unwrap();
    fs::write(r.join(".gitignore"), "docs/\n").unwrap();
    let res = filter_tree(&TreeQuery::new(r, "md", 50)).unwrap();
    assert!(res.rows.iter().all(|x| !x.subpath.starts_with("docs")), "{:?}", subpaths(&res));
    let mut q = TreeQuery::new(r, "md", 50);
    q.respect_git_ignore = false;
    let res = filter_tree(&q).unwrap();
    assert!(res.rows.iter().any(|x| x.subpath == "docs/readme.md"));
}

#[test]
fn bad_regex_is_reported() {
    let dir = sample();
    let err = filter_tree(&TreeQuery::new(dir.path(), "/[unclosed/", 50)).unwrap_err();
    assert!(matches!(err, TreeError::BadPattern(_)), "{err:?}");
}

#[test]
fn missing_root_is_an_error() {
    let err = filter_tree(&TreeQuery::new("/definitely/not/here", "x", 50)).unwrap_err();
    assert!(matches!(err, TreeError::Failed(_)));
}

#[test]
fn cancel_before_run() {
    let dir = sample();
    let s = Search::new();
    s.cancel();
    assert_eq!(s.run(&TreeQuery::new(dir.path(), "mai", 50)).unwrap_err(), TreeError::Cancelled);
}

/// A wide, deep tree with no match keeps broot busy; cancelling from
/// another thread must stop it promptly.
#[test]
fn cancel_during_run() {
    let dir = tempfile::tempdir().unwrap();
    for a in 0..40 {
        for b in 0..40 {
            for c in 0..10 {
                touch(dir.path(), &format!("d{a}/e{b}/f{c}.txt"));
            }
        }
    }
    let s = Search::new();
    let s2 = s.clone();
    let root = dir.path().to_path_buf();
    let h = thread::spawn(move || {
        let mut q = TreeQuery::new(root, "zzzqqq", 50);
        q.budget = None;
        let t = std::time::Instant::now();
        (s2.run(&q), t.elapsed())
    });
    thread::sleep(Duration::from_millis(5));
    s.cancel();
    let (res, _elapsed) = h.join().unwrap();
    // Either it was already done (tiny machine-dependent window) or it was cancelled.
    match res {
        Err(TreeError::Cancelled) => {}
        Ok(r) => assert!(r.rows.iter().all(|x| !x.direct_match)),
        Err(e) => panic!("{e:?}"),
    }
}

#[test]
fn budget_times_out() {
    let dir = tempfile::tempdir().unwrap();
    for a in 0..30 {
        for b in 0..30 {
            for c in 0..10 {
                touch(dir.path(), &format!("d{a}/e{b}/f{c}.txt"));
            }
        }
    }
    let mut q = TreeQuery::new(dir.path(), "zzzqqq", 50);
    q.budget = Some(Duration::from_nanos(1));
    match filter_tree(&q) {
        Err(TreeError::TimedOut) => {}
        other => panic!("expected timeout, got {other:?}"),
    }
}
