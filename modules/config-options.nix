{ lib, ... }:
{
  options = {
    model = lib.mkOption { type = lib.types.str; description = "Ollama model name (e.g. llama3, mistral, gemma3)"; example = "llama3"; };
    api_url = lib.mkOption { type = lib.types.str; default = "http://127.0.0.1:11434/api/generate"; description = "Ollama API endpoint"; };
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