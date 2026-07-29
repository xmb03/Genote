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
      provider = "ollama";
      model = "llama3";
      api_url = "http://127.0.0.1:11434";
      notes_dir = "/home/user/notes";
      lang = "ru";
      note_size = "small";
      notes_count = 7;
      use_covered_topics = true;
      weak_mode = true;

      # log = true;                                # show everything
      # log = { prompt = false; response = true; } # hide prompts, show responses

      # default = "work";
      # profile = {
      #   work = { notes_dir = "/home/user/work-notes"; note_size = "big"; weak_mode = true; };
      #   personal = { notes_dir = "/home/user/personal-notes"; lang = "ru"; note_size = "small"; model = "mistral"; };
      #   server = { api_url = "http://192.168.1.50:11434"; notes_dir = "/home/user/server-notes"; lang = "ru"; note_size = "big"; };
      #   cloud = { provider = "openai"; model = "gpt-4o"; api_url = "https://api.openai.com"; notes_dir = "/home/user/cloud-notes"; };
      # };
    };
  };
}