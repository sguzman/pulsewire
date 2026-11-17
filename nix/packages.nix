{ pkgs, craneLib }:
let
  lib = pkgs.lib;
  version = (builtins.fromTOML (builtins.readFile ../Cargo.toml)).workspace.package.version;

  # Keep compiled Cargo dependencies in a separate cacheable Nix derivation.
  # Crane's dummy dependency source prevents ordinary .rs edits from
  # invalidating this output. Both phases use the same pinned Rust toolchain.
  sharedArgs = {
    pname = "pulsewire";
    inherit version;
    src = ../.;
    cargoExtraArgs = "--workspace --bins --locked";
    doCheck = false; # Rust/container CI runs the integration suite.
    nativeBuildInputs = with pkgs; [ pkg-config cmake perl makeWrapper ];
    buildInputs = with pkgs; [ openssl sqlite ];
  };
  cargoArtifacts = craneLib.buildDepsOnly sharedArgs;

  # One Cargo workspace compilation still supplies all four executables.
  # Each source change reuses cargoArtifacts when dependencies are unchanged.
  all = craneLib.buildPackage (sharedArgs // {
    inherit cargoArtifacts;

    postInstall = ''
      # Match executable-relative config/schema lookup in config_paths.rs.
      mkdir -p "$out/share/pulsewire/config/fetcher/feeds" \
        "$out/share/pulsewire/config/server" \
        "$out/share/pulsewire/schemas"

      install -Dm644 crates/fetcher/res/config.toml \
        "$out/share/pulsewire/config/fetcher/config.toml"
      install -Dm644 crates/fetcher/res/domains.toml \
        "$out/share/pulsewire/config/fetcher/domains.toml"
      install -Dm644 crates/fetcher/res/categories.toml \
        "$out/share/pulsewire/config/fetcher/categories.toml"
      install -Dm644 crates/server/res/config.toml \
        "$out/share/pulsewire/config/server/config.toml"

      # Keep Nix's packaged catalog as narrow as the Docker image:
      # do not package disabled YouTube or podcast definitions.
      for category in news gov journals books finance tech; do
        if test -d "crates/fetcher/res/feeds/$category"; then
          cp -R "crates/fetcher/res/feeds/$category" \
            "$out/share/pulsewire/config/fetcher/feeds/"
        fi
      done

      cp -R crates/schemas/fetcher "$out/share/pulsewire/schemas/fetcher"
      cp -R crates/schemas/server "$out/share/pulsewire/schemas/server"
      cp -R crates/schemas/tui "$out/share/pulsewire/schemas/tui"
      cp -R crates/server/res/sql "$out/share/pulsewire/schemas/server/sql"

      cp -R crates/tui/res "$out/share/pulsewire/config/tui"
      wrapProgram "$out/bin/pulsewire-tui" \
        --set-default PULSEWIRE_TUI_CONFIG \
        "$out/share/pulsewire/config/tui/config.toml"
    '';

    meta = {
      description = "Time-aware feed ingestion, authenticated reader, CLI and TUI";
      homepage = "https://github.com/sguzman/pulsewire";
      license = lib.licenses.mit;
      platforms = lib.platforms.linux;
      mainProgram = "pulsewire";
    };
  });

  component = name: binary: pkgs.runCommand "pulsewire-${name}-${version}" { } ''
    mkdir -p "$out/bin" "$out/share"
    ln -s "${all}/bin/${binary}" "$out/bin/${binary}"
    ln -s "${all}/share/pulsewire" "$out/share/pulsewire"
  '';
in {
  cargo-deps = cargoArtifacts;
  default = all;
  pulsewire = all;
  fetcher = component "fetcher" "pulsewire";
  server = component "server" "pulsewire-server";
  cli = component "cli" "pulsewire-cli";
  tui = component "tui" "pulsewire-tui";
}
