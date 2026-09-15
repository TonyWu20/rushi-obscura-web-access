# rushi-web-access — ext flake (Mode B, ext-flake-authoring.md §3).
#
# Exposes two rushi tool packages:
#   packages.<system>.web-fetch   web_fetch   (obscura page fetch)
#   packages.<system>.web-search  web_search  (DDG search via obscura)
#
# $out contracts (ext-flake-authoring.md §2):
#   tool: $out/<tool>/tool.toml + $out/<tool>/bin/<binary>
#
# The obscura headless browser is bundled next to each tool binary
# ($out/<tool>/bin/obscura). The kernel's route resolves a tool binary
# from the tool's own bin/ dir (bin/route/src/main.rs), and the tool
# looks for obscura in its own exe dir before PATH, so the Nix-built
# package is self-contained: no PATH dependency on obscura.
#
# obscura is built from nixpkgs with the `render` build feature
# (screenshots) via overrideAttrs.
#
# Usage from a consumer flake (ext-flake-authoring.md §5):
#   inputs.rushi-web-access.url = "github:TonyWu20/rushi-web-access?ref=<rev>";
#   rushi.external_tools = [
#     webAccessFlake.packages.${system}."web-fetch"
#     webAccessFlake.packages.${system}."web-search"
#   ];

{
  description = "rushi-web-access — obscura-powered web_fetch and web_search tools for rushi";

  inputs = {
    # Pinned to the same nixpkgs/fenix revs as the other rushi ext
    # repos, so the toolchain builds are shared on the store.
    nixpkgs.url = "github:NixOS/nixpkgs/8ce4ef6cb6f871616146b9fe26d2a5ae594e94fe";
    fenix = {
      url = "github:nix-community/fenix/17757a67ed6621e70efc4271258825f34ee8409e";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, fenix }:
    let
      # Mirror the kernel flake: fixed system list + genAttrs.
      # Avoid flake-utils.eachDefaultSystem (transposes the result and
      # breaks nix develop / per-system devShells).
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      pkgLib = nixpkgs.lib;

      buildFor = system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ fenix.overlays.default ];
          };
          rustToolchain = fenix.packages.${system}.stable.withComponents [
            "cargo"
            "clippy"
            "rust-src"
            "rustc"
            "rustfmt"
            "rust-analyzer"
          ];

          # nixpkgs obscura with the `render` build feature enabled
          # (PNG screenshots via obscura fetch --screenshot).
          obscura = pkgs.obscura.overrideAttrs (final: prev: {
            cargoBuildFlags = (prev.cargoBuildFlags or [ ])
              ++ [ "--features" "render" ];
          });

          # Re-wrap the (structuredAttrs) obscura package into a plain
          # bundle derivation: $out/bin/obscura. A structuredAttrs
          # attrset breaks when listed in another derivation's
          # nativeBuildInputs, so the tool wrapper depends on this
          # plain bundle and copies its bin/ next to the tool binary.
          obscuraBundle = pkgs.stdenv.mkDerivation {
            pname = "obscura-bundle";
            version = "0";
            # this derivation only copies a built binary; stdenv still
            # wants a source for unpackPhase, so use a placeholder
            src = pkgs.writeTextFile {
              name = "obscura-bundle-src";
              destination = "/placeholder";
              text = "";
            };
            installPhase = ''
              mkdir -p $out/bin
              cp ${obscura}/bin/obscura $out/bin/obscura
            '';
          };

          # Build one standalone cargo crate from a subpath of this
          # flake's source tree. The whole repo is the source so the
          # intra-repo path dep (web-common) resolves in the sandbox;
          # buildAndTestSubdir cd's into the crate before cargo runs.
          # The output binary name comes from the crate's [[bin]] name.
          buildCrate = { crateDir, crateName }:
            pkgs.rustPlatform.buildRustPackage {
              pname = crateName;
              version = "0.1.0";
              src = "${self}";
              buildAndTestSubdir = crateDir;
              cargoRoot = crateDir;
              nativeBuildInputs = [ rustToolchain ];
              cargoLock = { lockFile = "${self}/${crateDir}/Cargo.lock"; };
              doCheck = false;
            };

          # Tool wrapper: $out/<name>/tool.toml + <name>/bin/<binary>.
          # The obscura binary is bundled into the same bin/ dir, so
          # the tool binary (which looks for obscura next to itself
          # before PATH) is self-contained in the Nix-built package.
          wrapAsTool = { name, toolToml, built }:
            pkgs.stdenv.mkDerivation {
              pname = "${name}-tool";
              version = "0.1.0";
              src = pkgs.writeTextFile {
                name = "${name}-tool-src";
                destination = "/placeholder";
                text = "";
              };
              nativeBuildInputs = [ built obscuraBundle ];
              installPhase = ''
                mkdir -p $out/${name}/bin
                cp -rL ${built}/bin/. $out/${name}/bin/
                cp -rL ${obscuraBundle}/bin/. $out/${name}/bin/
                cp ${toolToml} $out/${name}/tool.toml
              '';
            };
        in
        let
          # Hyphenated package names are quoted; plain let-bindings keep
          # `default` able to reference them.
          webFetchPkg = wrapAsTool {
            name = "web_fetch";
            toolToml = "${self}/web-fetch/tool.toml";
            built = buildCrate { crateDir = "web-fetch"; crateName = "web_fetch"; };
          };
          webSearchPkg = wrapAsTool {
            name = "web_search";
            toolToml = "${self}/web-search/tool.toml";
            built = buildCrate { crateDir = "web-search"; crateName = "web_search"; };
          };
        in
        {
          "web-fetch" = webFetchPkg;
          "web-search" = webSearchPkg;
          default = webFetchPkg;
        };
    in
    {
      # Standard flake shape: packages.<system>.<name>. `nix build .`
      # resolves packages.<host>.default, and consumers write
      # extFlake.packages.${system}."web-fetch".
      packages = pkgLib.genAttrs supportedSystems (system: buildFor system);

      # Dev shell: in-tree cargo dev with obscura on PATH.
      devShells = pkgLib.genAttrs supportedSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ fenix.overlays.default ];
          };
          rustToolchain = fenix.packages.${system}.stable.withComponents [
            "cargo"
            "rust-src"
            "rustc"
            "rust-analyzer"
          ];
        in
        {
          default = pkgs.mkShell {
            buildInputs = [ rustToolchain pkgs.obscura ];
            shellHook = ''
              echo "rushi-web-access dev shell: rust + obscura on PATH."
              echo "Build:  cd web-fetch && cargo build --release"
              echo "       cd ../web-search && cargo build --release"
            '';
          };
        }
      );
    };
}
