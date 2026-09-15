// web_fetch — fetch a web page through obscura and return its text.
//
// Rushi tool contract: one JSON object on stdin (the arguments), one
// JSON object on stdout (the result). On failure the message goes to
// stderr and the exit code is non-zero; the kernel forwards stderr to
// the model as the tool error.
//
// Arguments:
//   url        (string, required) http(s) URL
//   format     (string) text | markdown | links | html (default text)
//   max_lines  (int) cap on body lines (default 300)
//   wait_s     (int) extra settle wait for JS-heavy pages (default 0)
//
// Result:
//   { text, truncated, total_lines }

use web_common::*;

fn main() {
    let args = read_args();
    let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
    if !url.starts_with("http://") && !url.starts_with("https://") {
        fail(&format!("url must start with http:// or https:// (got: {url:?})"));
    }
    let raw_fmt = args.get("format").and_then(|v| v.as_str()).unwrap_or("text");
    let fmt = match raw_fmt {
        "text" | "markdown" | "links" | "html" => raw_fmt.to_string(),
        _ => "text".to_string(),
    };
    let max_lines = args
        .get("max_lines")
        .and_then(|v| v.as_i64())
        .unwrap_or(300)
        .max(1) as usize;
    let wait_s = args.get("wait_s").and_then(|v| v.as_i64()).unwrap_or(0).max(0);

    let mut obs_args: Vec<String> = vec![
        "fetch".into(),
        url.to_string(),
        "--dump".into(),
        fmt.clone(),
        "--timeout".into(),
        "30".into(),
        "--quiet".into(),
    ];
    if wait_s > 0 {
        obs_args.push("--wait".into());
        obs_args.push(wait_s.to_string());
    }

    let raw = match run_obscura(&obs_args) {
        Ok(o) => o,
        Err(e) => fail(&e),
    };

    let lines: Vec<&str> = raw.lines().collect();
    let total = lines.len();
    let (text, truncated) = if total > max_lines {
        let mut t = lines[..max_lines].join("\n");
        t.push_str(&format!(
            "\n\n--- [truncated at {max_lines} lines, {} more lines omitted] ---",
            total - max_lines
        ));
        (t, true)
    } else {
        (lines.join("\n"), false)
    };

    emit_json(&serde_json::json!({
        "text": text,
        "truncated": truncated,
        "total_lines": total,
    }));
}
