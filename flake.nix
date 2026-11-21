{
  description = "JEKO — 2.5D educational math game (Cerebras / Kimi K3.1 hackathon)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems f;

      # macroquad/miniquad links against these at build time and dlopens them
      # at runtime; alsa-lib is needed even though this vertical slice has no
      # audio yet, because macroquad initializes an audio device on startup.
      runtimeLibsFor = pkgs: pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux (with pkgs; [
        libGL
        libx11
        libxi
        libxcursor
        libxrandr
        libxinerama
        libxkbcommon
        alsa-lib
      ]);

      hasCargoLock = builtins.pathExists ./Cargo.lock;

      # openclaw is marked insecure in nixpkgs (full system access by
      # default + LLM parsing of untrusted content => prompt-injection
      # risk), so it gets its own pkgs instance with the allowance scoped
      # to that one derivation, rather than relaxing it for everything
      # evaluated via the default devShell's pkgs.
      pkgsInsecureFor = system: import nixpkgs {
        inherit system;
        config.permittedInsecurePackages = [
          "openclaw-2026.10.5"
        ];
      };
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          runtimeLibs = runtimeLibsFor pkgs;
        in
        {
          default = pkgs.mkShell {
            name = "jeko-dev";

            nativeBuildInputs = with pkgs; [
              rustc
              cargo
              rustfmt
              clippy
              rust-analyzer
              pkg-config
              secretspec
            ];

            # Also needed at compile time: nixpkgs' stdenv wraps the linker
            # so anything in buildInputs is discoverable, not just LD_LIBRARY_PATH.
            buildInputs = runtimeLibs;

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;

            shellHook = ''
              echo "jeko dev shell [''${JEKO_INSTANCE:-$(basename "$PWD")}] — $(rustc --version)"
              if command -v secretspec >/dev/null 2>&1 && [ -f secretspec.toml ]; then
                echo "secrets: 'secretspec check' then 'secretspec run -- cargo run' (see secretspec.toml)"
              fi
              if [ ! -f .env ] && [ -f .env.example ]; then
                echo "no .env found — cp .env.example .env and add CEREBRAS_API_KEY to enable live task generation (or use secretspec instead, see above)"
              fi
            '';
          };

          # Opt-in only: `nix develop .#openclaw`. Kept out of `default` on
          # purpose — see the pkgsInsecureFor comment above.
          openclaw = (pkgsInsecureFor system).mkShell {
            name = "jeko-openclaw";

            packages = [ (pkgsInsecureFor system).openclaw ];

            shellHook = ''
              echo "jeko openclaw shell — openclaw $(openclaw --version 2>/dev/null || echo unknown)"
              echo "nixpkgs marks this package insecure: it gives an LLM full system access by default and parses untrusted content, so it is vulnerable to prompt injection. Scope what it can touch before pointing it at anything sensitive."
            '';
          };
        });

      # Only exposed once Cargo.lock exists (buildRustPackage needs it to
      # pin/vendor crates). It doesn't exist yet because this flake was
      # authored on a machine with no Rust toolchain. Run `make build` (or
      # `cargo build` inside `nix develop`) once to generate it, then
      # `nix build` picks this up on the next evaluation — no edits needed.
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          runtimeLibs = runtimeLibsFor pkgs;
        in
        pkgs.lib.optionalAttrs hasCargoLock {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "jeko";
            version = "0.1.0";
            src = ./.;
            cargoLock = { lockFile = ./Cargo.lock; };

            nativeBuildInputs = with pkgs; [ pkg-config ]
              ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ patchelf ];
            buildInputs = runtimeLibs;

            postInstall = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
              patchelf --set-rpath "${pkgs.lib.makeLibraryPath runtimeLibs}:$(patchelf --print-rpath $out/bin/jeko)" $out/bin/jeko
            '';

            meta.mainProgram = "jeko";
          };
        });
    };
}
