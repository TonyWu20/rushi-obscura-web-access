# Research: lynx vs obscura for the rushi web-access tool

Date: 2026-09-16 (research session in rushi-web-access).

## Question

The pi-lynx extension (../pi-lynx) gives pi web access. It shells out to
lynx -dump for page fetch, DDG-Lite search, and old-Reddit search. It uses
native HTTP for JSON endpoints (Brave search, Reddit .json). It documents:
"JS-heavy pages are not supported."

Rushi has no equivalent tool yet. Both lynx and a headless browser binary,
obscura, are in the environment. Which engine should the rushi web-access
tool be built on?

## Environment facts (verified)

| | lynx | obscura |
|---|---|---|
| Version | 2.9.3 (26 May 2026) | 0.2.0 |
| Path | nix devshell bin | per-user bin |
| Model | terminal text browser, no JS | headless browser, V8 JS engine |
| Memory (static page) | ~30 MB peak | ~30 MB peak |

## pi-lynx design (the port target)

From ../pi-lynx/README.md and runtime.ts:

- lynx_web_fetch: run lynx -dump on the URL, split at the References line,
  strip [N] markers from the body, map [N] to URL from References, resolve
  duckduckgo.com/l/?uddg= redirects, cap body lines.
- lynx_web_search: build a DDG-Lite URL, lynx -dump, parse results.
- lynx_brave_search: native HTTP fetch of Brave server-rendered HTML.
- lynx_reddit_search: lynx -dump on old.reddit.com.
- lynx_reddit_fetch: native HTTP fetch of the Reddit .json thread endpoint.

So the engine question is: what replaces lynx -dump in a rushi port? And do
we keep lynx at all?

## Test results

All tests ran on this machine on 2026-09-16. Both binaries were invoked as
one-shot processes, the same way a rushi tool would.

### 1. Static page (example.com)

- lynx -dump: 0.12-0.15 s. Body on stdout. References block at the end.
  Clean [N] markers.
- obscura fetch --dump text: 0.06-0.15 s. Body on stdout. Diagnostics
  (Fetching, Page loaded) go to stderr. --dump markdown gives a heading,
  the body, and [Learn more](url).

Both are fine for static pages. Obscura is slightly faster. It also offers
a markdown dump that is directly model-readable.

### 2. JavaScript execution (the deciding test)

A local page whose DOM is only filled by a script tag:

    <div id="x">static-before</div>
    <script>document.getElementById("x").textContent = "JS-EXECUTED:" + 2+2;</script>

- lynx -dump: prints the raw HTML source, "static-before". No JS. On a
  real SPA (npmjs.com) it returns only "Enable JavaScript and cookies to
  continue".
- obscura fetch --dump text: prints "JS-EXECUTED:4", the post-JS DOM.
  --eval with JSON.stringify returned {"title":"Example Domain",
  "hasJS":"function"}.

Obscura executes JavaScript. Lynx cannot. This removes pi-lynx's
documented "JS-heavy pages not supported" limitation.

### 3. Search pages (DDG Lite)

- lynx: clean numbered results. References gives the [N] to URL map. The
  pi-lynx parser works on this output.
- obscura --dump text: works, but the region and time selector list
  renders as one noisy line. obscura --dump links: one URL-title tab pair
  per link, including all organic results. Both keep the
  duckduckgo.com/l/?uddg= redirects. That is the same resolveDdgRedirect
  step as pi-lynx.

For a structured web_search tool, the obscura links dump is easier to parse
than regex-ing the lynx text dump. It gives stable url-title rows with no
marker stripping.

### 4. Output quality and control

| Capability | lynx | obscura |
|---|---|---|
| Plain text dump | yes | yes |
| Markdown dump | no | yes (--dump markdown) |
| Links as structured pairs | via References parsing | native (--dump links) |
| Raw body and asset graph | no | --dump original, --dump assets |
| JS execution | no | yes |
| JS eval on page | no | yes (--eval) |
| Settle wait for SPAs | no | --wait (adaptive quiescence) |
| Cookie and session dump | no | --dump cookies |
| Anti-bot posture | basic | --stealth fingerprint and TLS impersonation |
| Batch multi-URL | no | scrape (concurrency 10, JSON out) |
| Screenshot to PNG | no | --screenshot (needs render build feature, not in the current binary) |

### 5. Error behavior against the rushi tool contract

Rushi contract: JSON on stdin, one JSON object on stdout. On non-zero
exit, stderr is forwarded to the model as the error message.

- obscura: content goes to stdout, diagnostics to stderr, exit 0 or 1,
  clean error text (Error: Failed to navigate ... Network error). Fits
  as-is.
- lynx: with -dump, content goes to stdout, stderr is mostly empty, exit
  codes OK. Also fits, but the wrapper must parse the References section
  and strip markers to get clean body text.

### 6. Cost

Static fetch: lynx 0.12-0.15 s, obscura 0.06-0.15 s.
JS page (react.dev, settled): obscura ~1.2-3 s.
Memory: both ~30 MB. Obscura with a JS page ~60 MB peak.

Both are cheap for a one-shot tool process. Rushi spawns tools per call,
so V8 startup cost is amortized per call, not per process.

## Head-to-head verdict

| Criterion | lynx | obscura |
|---|---|---|
| Static page fetch | good | good, plus markdown |
| JS-heavy pages | fails | works |
| Structured search results | good (References) | good (links dump, less noisy parse) |
| Fit to rushi tool ABI | needs marker stripping | clean stdout/stderr split |
| Extensions for agents | none | eval, wait, stealth, cookies, batch, screenshots |
| Maturity | 30+ years, rock solid | v0.2.0, young CLI |
| Failure modes | silent empty content on JS sites | bot walls still show interstitials even with stealth and wait |

## Decisions (settled 2026-09-16)

- Throttle: option (a) - the web_search tool keeps a state file
  ($HOME/.rushi/web-access/web_search_last) and sleeps the remaining
  3 s between calls. The sleep is capped at 10 s so a stale timestamp
  cannot hang the tool.
- Scope: v1 is web_fetch + web_search only. Brave and Reddit are out
  (never used on pi either, and both are unreliable here).
- Engine: obscura. Lynx stays out of the tools.

## Recommendation

Build the rushi web-access tool on obscura.

1. Obscura is a strict superset of what the tool needs. It does
   everything pi-lynx does with lynx -dump (fetch, DDG-Lite search,
   old-Reddit) plus JS execution. That deletes pi-lynx's biggest
   documented gap.
2. Its I/O split (content on stdout, diagnostics on stderr, clean exit
   codes) matches the rushi tool contract with zero glue.
3. The structured dumps (markdown, links) produce more agent-readable
   results than lynx's marker-laden text. The links dump gives parseable
   url-title pairs for a web_search tool.
4. --eval, --wait, --stealth, --dump cookies, and scrape batch are free
   capabilities for later tool variants. --screenshot (once the render
   build feature ships in the binary) can feed vision-capable models via
   the vision=true model config.
5. Costs are negligible either way. The decision is about capability, not
   performance.

Keep lynx out of the tool. It is a fallback, not an engine. If the tool
ever needs a JS-free fast path, a --engine lynx option is a cheap future
addition. The default and only engine at launch is obscura.

### Risks and mitigations

- Obscura is v0.2.0: pin the exact version in the flake and manifest.
  Treat the CLI as the single integration surface. The wrapper is the
  only thing that touches it. Re-run the prototype tests on version
  bumps.
- Bot walls: npmjs.com's Cloudflare check still returned an interstitial
  even with --stealth and a 45 s wait. This is network and IP specific,
  not engine specific. Pi-lynx documents the same class of failure for
  Brave and Reddit. The tool should surface the raw page text and let
  the model retry or pick another source.
- DDG-Lite noise: the obscura text dump of the search page includes the
  selector lists. Use the links dump as the parse target for web_search,
  not the text dump.
- Screenshot unavailable now: the installed binary lacks the render
  feature. Do not build tool behavior on --screenshot until the binary
  ships it.

## Search: what to port from pi-lynx

pi-lynx's search design: no API keys, public HTML search surfaces,
capped structured results (title, snippet, domain, URL), DDG `uddg`
redirect resolution, `site:` filters with 3 s spacing to dodge
throttling. That design is worth keeping. The implementation does not
need to mimic lynx-specific parsing.

What obscura changes:

- Parse surface: obscura `--dump links` on html.duckduckgo.com gives a
  stable per-result anchor group (title, bare, display-domain, snippet)
  all pointing at the same `uddg` URL. Verified: 10 results, each group
  intact. Parsing = group consecutive links by decoded `uddg` target.
  Strictly cleaner than lynx's text-dump `[N]` markers + References map.
- `uddg` resolution: pi-lynx's resolveDdgRedirect (URL-decode + drop the
  `rut` param) ports one to one.
- `site:` filter: works through obscura (`q=rust+site:gitlab.com`
  returned gitlab results). It is a plain query-string mechanism, so
  it is engine-independent.
- Throttling: pi-lynx spaces site-filtered searches in process memory.
  Rushi tools are one-shot, so the tool needs a small state file
  (last-search timestamp) to enforce spacing, or it surfaces the empty
  result and lets the model retry.
- Brave: its HTML is now a client-rendered SvelteKit app; the SSR
  `data-type="web"` blocks pi-lynx parses no longer exist. Obscura's
  headless fetch did not yield organic results within 8 s of wait, and
  data-center IPs are bot-checked. Do not build on Brave.
- Reddit: old.reddit.com search returned a login wall from this network.
  The Reddit `.json` thread fetch (plain HTTP) remains a possible
  add-on, but it is environment-dependent.

Search recommendation: ship one web_search tool (DDG html endpoint,
obscura links-dump parse, uddg resolution, optional site filter) plus
web_fetch (obscura). Leave Brave and Reddit out of v1.

## Tool shape (built)

Two Rust tool crates in this repo, each a standalone cargo package
with a tool.toml manifest (the single-repo ext pattern, mode B of
ext-flake-authoring.md):

- web_fetch {url, format: text|markdown|links|html, max_lines, wait_s}
  shells out to obscura fetch with the chosen dump format, caps the
  output, and returns {text, truncated, total_lines}.
- web_search {query, site, max_results}
  builds the html.duckduckgo.com URL (site filter, with github and
  wikipedia shortcuts), runs obscura with the links dump, groups the
  anchors per decoded uddg target into title / domain / snippet, and
  caps at max_results. A state file keeps a 3 s spacing between
  calls (one-shot tool, so there is no process memory to throttle).

Shared plumbing (obscura discovery, spawn, JSON I/O, uddg
resolution) lives in the web-common path-dep crate. JSON in/out goes
through serde_json directly, so no external jq is needed.

The ext flake (flake.nix) builds both crates, builds nixpkgs obscura
with the render build feature via overrideAttrs (PNG screenshots, for
a future vision tool result), and bundles the obscura binary next to
each tool binary ($out/<tool>/bin/obscura). The tool looks for
obscura in its own exe dir before PATH, so the Nix-built package is
self-contained. The kernel route resolves the tool binary from the
tool's own bin/ dir, so no PATH wiring is needed on the consumer
side.

Both crates pass the rushi ABI tests (JSON in, JSON out, stderr plus
non-zero exit on failure) on 2026-09-16, including the uddg grouping
parse and the state-file throttle.

## Nix packaging (ext flake)

The ext flake (flake.nix, Mode B) builds both tool crates, the
nixpkgs obscura with the `render` build feature (overrideAttrs adds
`--features render` to cargoBuildFlags; screenshots verified on the
Nix-built binary: 1280x720 PNG via `--screenshot`), and a bundle
derivation that re-wraps obscura into a plain $out/bin/obscura.
Each tool package is the P1 layout ($out/<tool>/tool.toml +
bin/<tool> + bin/obscura).

Nixpkgs quirks hit on the pinned rev (8ce4ef6..., noted so the
kernel guide can absorb them):

- structuredAttrs: listing the nixpkgs obscura package (a
  structuredAttrs attrset) in another derivation's nativeBuildInputs
  or buildEnv.paths fails with `error: attribute 'name' missing`
  in derivationStrict. Plain mkDerivation/buildRustPackage outputs
  list fine. Workaround: the obscuraBundle re-wrap.
- mkDerivation name computation: this rev only synthesizes `name`
  when `name` is passed or when BOTH pname and version are present
  (make-derivation.nix: `${if (attrs ? name || (attrs ? pname &&
  attrs ? version)) then "name" else null}`). The guide's
  wrapAsTool skeleton (pname only) fails; add version = "0.1.0".
- stdenv requires a source: bare-copy mkDerivations need a `src`
  (placeholder via writeTextFile) or unpackPhase dies with "variable
  $src or $srcs should point to the source".
- intra-repo path deps: buildCrate uses src = "${self}" (whole
  repo, as goal-app does) plus buildAndTestSubdir/cargoRoot, so
  web-common resolves in the sandbox.

The kernel route (bin/route/src/main.rs) resolves a tool binary
from the tool's own bin/ dir, and mk-rushi.nix copies external
tools additively, so the bundled obscura survives into the rushi
package: no PATH wiring on the consumer side.

## Feedback filed

The nixpkgs quirks above are filed as
[TonyWu20/rushi#8](https://github.com/TonyWu20/rushi/issues/8),
suggesting the ext-flake-authoring skeleton be updated.
