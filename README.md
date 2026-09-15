# rushi-web-access

Obscura-powered web tools for rushi: `web_fetch` and `web_search`.

Both tools are one-shot Rust binaries that speak the rushi tool ABI
(JSON on stdin, one JSON object on stdout, non-zero exit plus stderr
on failure) and shell out to the obscura headless browser, which is
bundled into the Nix-built packages.

## Tools

### web_fetch

Fetch a page (JavaScript is executed first) and return its content.

| arg         | type   | default | meaning                                             |
|-------------|--------|---------|-----------------------------------------------------|
| url         | string | required | http(s) URL to fetch                                |
| format      | string | text    | text, markdown, links or html (obscura dump format) |
| max_lines   | number | 400     | cap on returned lines                                 |
| wait_s      | number | 0       | settle time for slow / JS-heavy pages               |

Output: `{text, truncated, total_lines}`.

### web_search

Keyless web search via the DuckDuckGo html endpoint (no API key, no
scraping of client-rendered pages). Results are grouped per result
title / domain / snippet and their redirect targets are resolved.

| arg         | type   | default | meaning                                             |
|-------------|--------|---------|-----------------------------------------------------|
| query       | string | required | search query                                          |
| site        | string | -       | restrict to a site; `github` and `wikipedia` are shortcuts |
| max_results | number | 5       | cap on returned results                               |

Output: `{results: [{title, url, domain, snippet}], count}`.
Back-to-back calls are paced (3 s minimum gap) through a state file,
so the model does not hammer the endpoint.

## Crates

- `web-common` shared plumbing: obscura discovery (exe dir first, then
  PATH), process spawn, JSON I/O, url and uddg redirect resolution.
- `web-fetch` the `web_fetch` tool binary.
- `web-search` the `web_search` tool binary.

Each tool crate carries its `tool.toml` manifest. In-tree cargo dev:

    cd web-fetch && cargo build --release
    cd ../web-search && cargo build --release

Or the Nix dev shell: `nix develop` (rust toolchain plus obscura on
PATH).

## Nix packaging

`flake.nix` is a Mode B ext flake (see
`rust-unix-harness/docs/reference/nix/ext-flake-authoring.md`). It
exposes two packages per system:

    packages.<system>."web-fetch"
    packages.<system>."web-search"

Each package's `$out` is the P1 tool layout:

    $out/web_fetch/tool.toml
    $out/web_fetch/bin/web_fetch
    $out/web_fetch/bin/obscura

The obscura binary is built from nixpkgs with the `render` cargo
feature (PNG screenshots) and re-wrapped into a plain bundle
derivation, because the structuredAttrs obscura package breaks when
listed in another derivation's `nativeBuildInputs` on the pinned
nixpkgs rev. The tool binary looks for obscura next to itself before
PATH, so the package is self-contained.

Consumer wiring (ext-flake-authoring.md section 5):

    inputs.rushi-web-access = {
      url = "github:TonyWu20/rushi-web-access";
      flake = true;
    };

    # in the rushi module:
    rushi.external_tools = [
      webAccessFlake.packages.${system}."web-fetch"
      webAccessFlake.packages.${system}."web-search"
    ];

## Notes

- See `docs/research-web-engine.md` for the engine choice (obscura
  over lynx) and the search-endpoint research (DuckDuckGo html
  endpoint, why Brave and Reddit are out of scope for v1).
