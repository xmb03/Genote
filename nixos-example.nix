# NixOS example for Genote
# Add to your configuration.nix:
#   imports = [ ./nixos-example.nix ];
# Then: sudo nixos-rebuild switch

{ config, lib, pkgs, inputs, ... }:
{
  imports = [ inputs.genote.nixosModules.default ];

  programs.genote = {
    enable = true;
    settings = {
      model = "llama3";
      api_url = "http://127.0.0.1:11434/api/generate";
      notes_dir = "/home/user/notes";
      lang = "ru";
      note_size = "small";
      notes_count = 7;
      use_covered_topics = true;

      # default = "work";
      # profile = {
      #   work = { notes_dir = "/home/user/work-notes"; note_size = "big"; };
      #   personal = { notes_dir = "/home/user/personal-notes"; lang = "ru"; note_size = "small"; model = "mistral"; };
      # };
    };
  };
}