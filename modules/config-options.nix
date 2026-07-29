{ lib, ... }:
{
  options = {
    provider = lib.mkOption { type = lib.types.enum [ "ollama" "openai" "llamacpp" "anthropic" "gemini" ]; default = "ollama"; description = "API provider"; };
    model = lib.mkOption { type = lib.types.str; description = "Model name (e.g. llama3, gpt-4o, claude-sonnet-4, gemini-2.0-flash)"; example = "llama3"; };
    api_url = lib.mkOption { type = lib.types.str; default = "http://127.0.0.1:11434"; description = "API endpoint (base URL without path suffix)"; };
    notes_dir = lib.mkOption { type = lib.types.path; description = "Directory containing existing .md notes for style examples"; example = "~/notes"; };
    lang = lib.mkOption { type = lib.types.str; default = "en"; description = "Language for generated notes (en, ru, etc.)"; };
    note_size = lib.mkOption { type = lib.types.enum [ "small" "big" ]; default = "small"; description = "Note size: small (25-30 lines) or big (comprehensive)"; };
    notes_count = lib.mkOption { type = lib.types.int; default = 7; description = "Number of example notes to use"; };
    use_covered_topics = lib.mkOption { type = lib.types.bool; default = false; description = "Restrict model to concepts from existing note filenames"; };
    weak_mode = lib.mkOption { type = lib.types.bool; default = false; description = "Two-stage generation for models <15B. Slower, better style cloning"; };
    log = lib.mkOption {
      type = lib.types.nullOr lib.types.anything;
      default = null;
      description = ''
        Logging control. Three modes:
        - `true` — show everything (prompts, responses, status, timing)
        - `false` — suppress all non-error output
        - attrset e.g. `{ prompt = false; response = true; }` — per-category control
      '';
      example = { prompt = false; response = true; };
    };
    default = lib.mkOption { type = lib.types.nullOr lib.types.str; default = null; description = "Default profile name when --profile not specified";
};
}