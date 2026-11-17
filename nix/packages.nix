{ pkgs, cargoNix }:
let
  lib = pkgs.lib;
  version = (builtins.fromTOML (builtins.readFile ../Cargo.toml)).workspace.package.version;

  # Each workspace member and each upstream dependency is a separate
  # crate2nix Nix derivation. Unchanged third-party crates are reusable.
  builds = {
    fetcher = cargoNix.workspaceMembers."pulsewire-fetcher".build;
    server = cargoNix.workspaceMembers."pulsewire-server".build;
    cli = cargoNix.workspaceMembers."pulsewire-cli".build;
    tui = cargoNix.workspaceMembers."pulsewire-tui".build;
  };

  # Assemble the four binaries and their runtime assets without rebuilding
  # any Rust dependency inside this derivation.
  all = pkgs.runCommand "pulsewire-${version}" {
    nativeBuildInputs = [ pkgs.makeWrapper ];
  } ''
    # Copy only the final binaries. Linux current_exe() resolves symlinks,
    # so pointing directly to workspace-member store paths would break
    # executable-relative config and schema discovery in the assembled output.
    install -Dm755 "${builds.fetcher}/bin/pulsewire" "$out/bin/pulsewire"
    install -Dm755 "${builds.server}/bin/pulsewire-server" "$out/bin/pulsewire-server"
    install -Dm755 "${builds.cli}/bin/pulsewire-cli" "$out/bin/pulsewire-cli"
    install -Dm755 "${builds.tui}/bin/pulsewire-tui" "$out/bin/pulsewire-tui"
    cd ${../.}

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
      # The source directory is read-only in /nix/store. Keep the copied
      # schema tree writable until server SQL migrations are installed.
      cp -R --no-preserve=mode crates/schemas/server "$out/share/pulsewire/schemas/server"
      cp -R crates/schemas/tui "$out/share/pulsewire/schemas/tui"
      cp -R crates/server/res/sql "$out/share/pulsewire/schemas/server/sql"

      cp -R crates/tui/res "$out/share/pulsewire/config/tui"
      wrapProgram "$out/bin/pulsewire-tui" \
        --set-default PULSEWIRE_TUI_CONFIG \
        "$out/share/pulsewire/config/tui/config.toml"
  '';

  component = name: binary: pkgs.runCommand "pulsewire-${name}-${version}" { } ''
    mkdir -p "$out/bin" "$out/share"
    ln -s "${all}/bin/${binary}" "$out/bin/${binary}"
    ln -s "${all}/share/pulsewire" "$out/share/pulsewire"
  '';
in {
  default = all;
  pulsewire = all;
  fetcher = component "fetcher" "pulsewire";
  server = component "server" "pulsewire-server";
  cli = component "cli" "pulsewire-cli";
  tui = component "tui" "pulsewire-tui";
}
