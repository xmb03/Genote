# Home Manager example for Genote
# Add to your home.nix:
#   imports = [ ./home-manager-example.nix ];
# Then: home-manager switch

{ config, lib, pkgs, inputs, ... }:
{
  imports = [ inputs.genote.homeManagerModules.default ];

  programs.genote = {
    enable = true;
    settings = {
      provider = "ollama";
      model = "llama3";
      api_url = "http://127.0.0.1:11434";
      notes_dir = "~/notes";
      lang = "ru";
      note_size = "small";
      notes_count = 7;
      use_covered_topics = true;
      weak_mode = true;

      # log = true;                                # show everything
      # log = { prompt = false; response = true; } # hide prompts, show responses

      # default = "work";
      # profile = {
      #   work = { notes_dir = "~/work-notes"; note_size = "big"; notes_count = 10; use_covered_topics = true; weak_mode = true; };
      #   personal = { notes_dir = "~/personal-notes"; lang = "ru"; note_size = "small"; model = "mistral"; use_covered_topics = false; };
      #   server = { api_url = "http://192.168.1.50:11434"; notes_dir = "~/server-notes"; lang = "ru"; note_size = "big"; };
      #   cloud = { provider = "openai"; model = "gpt-4o"; api_url = "https://api.openai.com"; notes_dir = "~/cloud-notes"; };
      # };
    };
  };
}