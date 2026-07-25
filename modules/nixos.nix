{ lib, pkgs, config, ... }:
let cfg = config.programs.genote;
in {
  options.programs.genote = {
    enable = lib.mkEnableOption "Genote CLI for generating IT study notes (system-wide)";
    package = lib.mkOption { type = lib.types.package; default = pkgs.genote; description = "Genote package to use"; };
    settings = lib.mkOption { type = lib.types.submodule (import ./config-options.nix { inherit lib; }); default = { }; description = "Genote configuration (written to /etc/genote/config.toml)"; };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    environment.etc."genote/config.toml".text = lib.toToml cfg.settings;
  };
}