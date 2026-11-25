{
  description = "auth — FastTrackStudio's identity library and its server (auth.fasttrackstudio.app)";

  inputs = {
    # The fleet's pins: nixpkgs, and `lib.toolchainFor` for the fleet's
    # rustc (rust-toolchain.toml).
    devenv-fleet.url = "github:FastTrackStudios/devenv-fleet/c4ddec850ad97b5da74c746bff39a988f02b13a4";
    nixpkgs.follows = "devenv-fleet/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      devenv-fleet,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ devenv-fleet.overlays.default ];
        };
        toolchain = devenv-fleet.lib.toolchainFor pkgs;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };

        auth-server = rustPlatform.buildRustPackage {
          pname = "auth-server";
          version = "0.1.0";
          src = ./.;
          cargoBuildFlags = [
            "-p"
            "auth-server"
          ];

          cargoLock = {
            lockFile = ./Cargo.lock;
            # One hash per git source (url + rev), keyed by the
            # alphabetically first crate it provides. When a pin moves the
            # build fails with the expected value: paste what it prints.
            outputHashes = {
              "const-serialize-0.8.0-alpha.3" = "sha256-ZWqLFm5apsJu24ishMsilTkndLrlRmb0JQRyVXSBt50=";
              "facet-core-0.50.0-rc.7" = "sha256-Tew1a4NAztMxnDGQMzwzBVaNl+L9BOFrB+Dpw9WgfnU=";
              "phon-jit-0.2.0-rc.7" = "sha256-5a0PwCfBdJjV06rv2Nn5x8VnX7mx5/oPl0IiDne/OUI=";
              "vox-phon-0.10.0-rc.7" = "sha256-aYDaHTS2NHVK1sp/yZwbzM2WNUsIzmtOwJdvUE46iUk=";
            };
          };

          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.removeReferencesTo
          ];
          buildInputs = [ pkgs.openssl ];

          # The round-trip tests bind sockets; they run in CI, not in a
          # sandboxed package build.
          doCheck = false;

          # std's panic locations name the toolchain's rust-src, which
          # would keep the whole toolchain in the runtime closure.
          postFixup = ''
            remove-references-to -t ${toolchain} $out/bin/auth-server
          '';
          disallowedReferences = [ toolchain ];

          meta.mainProgram = "auth-server";
        };
      in
      {
        packages = {
          inherit auth-server;
          default = auth-server;
        };

        devShells.default = pkgs.mkShell {
          packages = [
            toolchain
            pkgs.pkg-config
            pkgs.openssl
            pkgs.postgresql
            pkgs.tailwindcss_4
            pkgs.just
          ];
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
