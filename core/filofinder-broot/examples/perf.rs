//! Rough performance check: builds (or reuses) a synthetic tree of ~100k
//! files and times a few broot queries through filofinder-broot.
//!
//!     cargo run --release -p filofinder-broot --example perf -- [/tmp/ff-perf-tree] [max_rows]

use std::{fs, path::Path, time::Instant};

use filofinder_broot::{Search, TreeQuery};

const WORDS: &[&str] = &[
    "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliet",
];
const EXTS: &[&str] = &["rs", "md", "txt", "swift", "json"];

fn build(root: &Path) -> usize {
    let mut n = 0;
    for a in 0..10 {
        for b in 0..10 {
            for c in 0..10 {
                let dir = root.join(format!("{}_{a}/{}_{b}/{}_{c}", WORDS[a], WORDS[b], WORDS[c]));
                fs::create_dir_all(&dir).unwrap();
                for f in 0..100 {
                    let name = format!("{}_{}_{f}.{}", WORDS[(a + f) % 10], WORDS[(b * 3 + f) % 10], EXTS[f % 5]);
                    fs::write(dir.join(name), b"").unwrap();
                    n += 1;
                }
            }
        }
    }
    n
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = Path::new(args.get(1).map(String::as_str).unwrap_or("/tmp/ff-perf-tree")).to_path_buf();
    let max_rows: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(60);
    if !root.join(".complete").exists() {
        let t = Instant::now();
        let n = build(&root);
        fs::write(root.join(".complete"), b"").unwrap();
        eprintln!("built {n} files in {:?}", t.elapsed());
    }
    // Warm-up (broot context init + page cache).
    let _ = Search::new().run(&TreeQuery::new(&root, "warmup", max_rows));
    let queries = [
        "a", "echo", "jul_ind", "zzzz_nomatch", "/delta_[0-9]+\\.rs$/", "p/golf_3/hotel", "foxtrot_juliet_42.md",
    ];
    println!("{:<26} {:>9} {:>6} {:>7}", "query", "ms", "rows", "matches");
    for q in queries {
        let mut times = Vec::new();
        let mut last = None;
        for _ in 0..5 {
            let t = Instant::now();
            let r = Search::new().run(&TreeQuery::new(&root, q, max_rows)).unwrap();
            times.push(t.elapsed().as_secs_f64() * 1000.0);
            last = Some(r);
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let r = last.unwrap();
        let matches = r.rows.iter().filter(|x| x.direct_match).count();
        println!("{:<26} {:>9.1} {:>6} {:>7}", q, times[2], r.rows.len(), matches);
    }
    // Cancellation latency on the slowest (no-match) query.
    let s = Search::new();
    let s2 = s.clone();
    let root2 = root.clone();
    let t = Instant::now();
    let h = std::thread::spawn(move || {
        let mut q = TreeQuery::new(root2, "zzzz_nomatch", max_rows);
        q.budget = None;
        s2.run(&q)
    });
    std::thread::sleep(std::time::Duration::from_millis(20));
    let tc = Instant::now();
    s.cancel();
    let res = h.join().unwrap();
    println!(
        "cancel after 20 ms: {:?}, returned {:.1} ms after cancel (total {:.1} ms)",
        res.err(),
        tc.elapsed().as_secs_f64() * 1000.0,
        t.elapsed().as_secs_f64() * 1000.0
    );
}
