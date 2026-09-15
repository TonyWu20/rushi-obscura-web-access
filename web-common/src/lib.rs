// Shared plumbing for the rushi web tools: obscura discovery and
// spawn, tool-argument I/O, URL helpers, and the DDG uddg redirect
// resolver (ported from pi-lynx resolveDdgRedirect).

use std::path::PathBuf;
use std::process::Command;

/// Locate the obscura binary. A bundled copy in the tool's own bin
/// dir (placed there by the ext flake) wins; otherwise fall back to
/// a bare PATH lookup.
pub fn find_obscura() -> Result<PathBuf, String> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("obscura");
            if bundled.is_file() {
                return Ok(bundled);
            }
        }
    }
    Ok(PathBuf::from("obscura"))
}

/// Run obscura with the given args. On success return stdout as a
/// String. On failure forward the error text (the caller writes it
/// to stderr and exits non-zero, per the rushi tool contract).
pub fn run_obscura(args: &[String]) -> Result<String, String> {
    let bin = find_obscura()?;
    let out = Command::new(&bin)
        .args(args)
        .output()
        .map_err(|e| format!("failed to spawn obscura ({:?}): {e}", bin))?;
    if !out.status.success() {
        let mut msg = format!("obscura exited with {}", out.status);
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        if !err.is_empty() {
            msg.push_str(": ");
            msg.push_str(err);
        }
        return Err(msg);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Read the single JSON arguments object from stdin.
pub fn read_args() -> serde_json::Value {
    let mut s = String::new();
    use std::io::Read;
    if std::io::stdin().read_to_string(&mut s).is_err() {
        s = String::new();
    }
    serde_json::from_str(&s).unwrap_or(serde_json::Value::Null)
}

/// Emit the single JSON result object on stdout.
pub fn emit_json(v: &serde_json::Value) {
    let s = serde_json::to_string(v).unwrap_or_default();
    println!("{s}");
}

/// Print the failure message to stderr and exit non-zero.
pub fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(1);
}

/// Percent-encode a string for use inside a URL query value.
pub fn uri_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Decode percent-escapes in a byte string.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Resolve a DuckDuckGo redirect URL (duckduckgo.com/l/?uddg=<enc>&rut=...)
/// to its real target URL. Non-DDG URLs are returned unchanged.
/// Port of pi-lynx resolveDdgRedirect.
pub fn resolve_uddg(url: &str) -> String {
    const PREFIX: &str = "duckduckgo.com/l/?uddg=";
    let Some(pos) = url.find(PREFIX) else {
        return url.to_string();
    };
    let encoded = &url[pos + PREFIX.len()..];
    let encoded = encoded.split('&').next().unwrap_or("");
    percent_decode(encoded)
}
