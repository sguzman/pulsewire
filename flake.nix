{
  description = "Pulsewire: reproducible Rust builds, development and local QA";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forEachSystem = nixpkgs.lib.genAttrs systems;

      forSystem = system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          # rust-overlay is pinned by flake.lock. The repository's existing
          # rust-toolchain.toml selects the components/channel, including nightly.
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };
          packages = import ./nix/packages.nix {
            inherit pkgs rustPlatform;
          };
          commonTools = with pkgs; [
            rustToolchain
            pkg-config
            cmake
            perl
            openssl
            sqlite
            jq
            just
            nodejs
            nixfmt
          ];
          mkShell = extra: pkgs.mkShell {
            packages = commonTools ++ extra;
            shellHook = ''
              export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"
            '';
          };
        in {
          inherit pkgs packages;
          devShells = {
            default = mkShell [ ];
            qa = mkShell (with pkgs; [
              postgresql_16
              chromium
            ]);
          };
        };
    in {
      packages = forEachSystem (system: (forSystem system).packages);
      devShells = forEachSystem (system: (forSystem system).devShells);
      formatter = forEachSystem (system: (forSystem system).pkgs.nixfmt);
      checks = forEachSystem (system: {
        package = (forSystem system).packages.default;
      });
      apps = forEachSystem (system:
        let
          p = (forSystem system).packages;
          app = package: binary: {
            type = "app";
            program = "${package}/bin/${binary}";
          };
        in {
          default = app p.fetcher "pulsewire";
          fetcher = app p.fetcher "pulsewire";
          server = app p.server "pulsewire-server";
          cli = app p.cli "pulsewire-cli";
          tui = app p.tui "pulsewire-tui";
        });
    };
}
