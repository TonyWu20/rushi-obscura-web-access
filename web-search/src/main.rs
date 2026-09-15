// web_search — keyless web search via DuckDuckGo's HTML endpoint,
// rendered by obscura and parsed from its structured links dump.
//
// Rushi tool contract: one JSON object on stdin (the arguments), one
// JSON object on stdout (the result). On failure the message goes to
// stderr and the exit code is non-zero; the kernel forwards stderr to
// the model as the tool error.
//
// Arguments:
//   query        (string, required)
//   site         (string) optional site filter; "github"/"wikipedia"
//                are shortcuts for github.com / wikipedia.org
//   max_results  (int) cap on results (default 8)
//
// Result:
//   { query, results: [{ title, url, domain, snippet }], count }
// or, when nothing comes back:
//   { query, results: [], count: 0, note }
//
// The links dump of the DDG HTML page yields, per organic result, a
// group of anchors (title, bare, display domain, snippet) all pointing
// at the same duckduckgo.com/l/?uddg=... redirect URL. Grouping the
// anchors by decoded uddg target recovers the result structure.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use web_common::*;

fn main() {
    let args = read_args();
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    if query.is_empty() {
        fail("missing required field: query");
    }
    let mut site = args.get("site").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_i64())
        .unwrap_or(8)
        .max(1) as usize;

    // site shortcuts, parity with pi-lynx bangs (!gh, !w)
    match site.as_str() {
        "github" => site = "github.com".into(),
        "wikipedia" => site = "wikipedia.org".into(),
        _ => {}
    }

    throttle();

    let mut q = uri_encode(query);
    if !site.is_empty() {
        q.push_str(&format!("+site:{}", uri_encode(&site)));
    }
    let url = format!("https://html.duckduckgo.com/html/?q={q}");

    let raw = match run_obscura(&[
        "fetch".into(),
        url,
        "--dump".into(),
        "links".into(),
        "--timeout".into(),
        "30".into(),
        "--quiet".into(),
    ]) {
        Ok(o) => o,
        Err(e) => fail(&e),
    };

    let results = parse_ddg_links(&raw, max_results);
    if results.is_empty() {
        emit_json(&serde_json::json!({
            "query": query,
            "results": [],
            "count": 0,
            "note": "no results returned (possibly throttled or blocked; retry later)"
        }));
        return;
    }
    emit_json(&serde_json::json!({
        "query": query,
        "results": results,
        "count": results.len(),
    }));
}

/// Parse the obscura links dump of a DDG HTML search page into
/// structured results. Each line is `url<TAB>anchor`. Organic result
/// anchors all point at the same uddg redirect URL; consecutive lines
/// with the same decoded target form one result (title first, then
/// the display domain, then the longest remaining anchor as snippet).
fn parse_ddg_links(raw: &str, max_results: usize) -> Vec<serde_json::Value> {
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for line in raw.lines() {
        let (url, anchor) = match line.split_once('\t') {
            Some((u, a)) => (u, a.trim()),
            None => continue,
        };
        if !url.contains("duckduckgo.com/l/?uddg=") {
            continue;
        }
        let target = resolve_uddg(url);
        if let Some(last) = groups.last_mut() {
            if last.0 == target {
                if !anchor.is_empty() {
                    last.1.push(anchor.to_string());
                }
                continue;
            }
        }
        let mut anchors = Vec::new();
        if !anchor.is_empty() {
            anchors.push(anchor.to_string());
        }
        groups.push((target, anchors));
    }

    let mut results = Vec::new();
    for (target, anchors) in groups {
        let title = anchors.first().cloned().unwrap_or_default();
        let mut domain = String::new();
        let mut snippet = String::new();
        for a in &anchors[1..] {
            if a.chars().all(|c| !c.is_whitespace()) && domain.is_empty() {
                domain = a.clone();
            } else if a.len() > snippet.len() {
                snippet = a.clone();
            }
        }
        let snippet: String = snippet.chars().take(300).collect();
        results.push(serde_json::json!({
            "title": title,
            "url": target,
            "domain": domain,
            "snippet": snippet,
        }));
        if results.len() >= max_results {
            break;
        }
    }
    results
}

/// Keep at least 3 s between two searches. Rushi tools are one-shot
/// processes, so the last-search timestamp lives in a small state
/// file. The sleep is capped at 10 s so a stale or far-past timestamp
/// cannot hang the tool.
fn throttle() {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return,
    };
    let dir = PathBuf::from(home).join(".rushi").join("web-access");
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let state = dir.join("web_search_last");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let wait = fs::read_to_string(&state)
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .map(|last| (3 - (now - last)).max(0).min(10))
        .unwrap_or(0);
    if wait > 0 {
        std::thread::sleep(Duration::from_secs(wait as u64));
    }
    let _ = fs::write(&state, now.to_string());
}
