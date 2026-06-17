{
  description = "feedback-cli — a Rust library that routes structured CLI error/feedback/perf events to a configurable strategy (caco webhook, caco CLI, or a local sink), built on the mcp-cli stack.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    # Ecosystem crate pulled in directly from github:harryaskham/* — NOT
    # vendored. `nix flake update` moves it in lockstep with everything else. We
    # consume the source (flake = false) and wire it into the cargo build via
    # `[patch]` so the build is fully offline/reproducible inside the nix sandbox.
    #
    # Private repo, fetched over SSH (git+ssh://) — nix's git fetcher uses the
    # host's git/SSH key auth, no GitHub token required.
    mcp-cli = {
      url = "git+ssh://git@github.com/harryaskham/mcp-cli?ref=main";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      mcp-cli,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;

        # `[patch]` redirects every `https://github.com/harryaskham/mcp-cli` git
        # reference in the dependency graph to the pinned flake-input source tree.
        cargoConfig = pkgs.writeText "feedback-cli-cargo-config.toml" ''
          [patch."https://github.com/harryaskham/mcp-cli"]
          mcp-cli = { path = "${mcp-cli}" }
        '';

        feedbackCli = pkgs.rustPlatform.buildRustPackage {
          pname = "feedback-cli";
          version = "0.1.0";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          # Inject the [patch] table so cargo resolves the ecosystem crate from
          # the flake input instead of reaching out to the network.
          postPatch = ''
            mkdir -p .cargo
            cat ${cargoConfig} >> .cargo/config.toml
          '';

          # This is a library: there is no binary to install, but `cargo test`
          # (run by buildRustPackage's checkPhase) is the primary gate, covering
          # the unit tests AND the rustdoc doctests.
          doCheck = true;

          meta = {
            description = "Configurable error/feedback/perf reporting for CLIs built on the harryaskham mcp-cli stack";
            license = lib.licenses.mit;
          };
        };
      in
      {
        packages.default = feedbackCli;
        packages.feedback-cli = feedbackCli;

        # `nix run .#doctor` verifies the common project conventions are ON
        # (workflows present + right runners, .envrc, version agreement, lib
        # shape). For a library crate there is no service module to evaluate, so
        # the doctor runs in --lib mode.
        apps.doctor = {
          type = "app";
          program = "${pkgs.writeShellScript "feedback-cli-doctor" ''
            export PATH="${
              lib.makeBinPath [
                pkgs.git
                pkgs.curl
                pkgs.nix
              ]
            }:$PATH"
            exec ${pkgs.bash}/bin/bash ${./scripts/doctor.sh} "$@"
          ''}";
        };

        # `nix run .#release -- {major|minor|patch|X.Y.Z}` bumps the version
        # across Cargo.toml/Cargo.lock/flake.nix, commits, and pushes a v* tag,
        # which triggers release.yml. A library publishes to crates.io / a git
        # tag rather than shipping binaries.
        apps.release = {
          type = "app";
          program = "${pkgs.writeShellScript "feedback-cli-release" ''
            export PATH="${
              lib.makeBinPath [
                pkgs.git
                pkgs.cargo
              ]
            }:$PATH"
            exec ${pkgs.bash}/bin/bash ${./scripts/release.sh} "$@"
          ''}";
        };

        # Sandbox-pure check: the library builds and its tests (unit + doctest)
        # pass.
        checks.test = feedbackCli;

        devShells.default = pkgs.mkShell {
          inputsFrom = [ feedbackCli ];
          packages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
          ];
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
