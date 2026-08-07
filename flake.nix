{
  description = "Genote - Generate IT study notes using local LLMs and cloud APIs";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, home-manager, ... }@inputs:
    let
      forEachSystem = flake-utils.lib.eachDefaultSystem (system:
        let
          pkgs = import nixpkgs { inherit system; };
          lib = pkgs.lib;
          genote = pkgs.rustPlatform.buildRustPackage rec {
            pname = "genote";
            version = "0.4.1";
            src = lib.cleanSourceWith {
              filter = f: t: t != "target" && t != ".git" && t != ".playwright-mcp" && t != ".opencode/node_modules";
              src = self;
            };
            cargoSha256 = "sha256-fAqfDHYqYvtL0IZNee2+ePb3xxX/eTVt5pTdNr4uYEI=";
            cargoLock = { lockFile = ./Cargo.lock; };
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];
          };
        in {
          packages = { genote = genote; default = genote; };
          devShells.default = pkgs.mkShell { buildInputs = [ pkgs.cargo pkgs.rustc pkgs.openssl pkgs.pkg-config ]; };
        });

      hmModule = import ./modules/home-manager.nix {
        inherit (import nixpkgs { system = "x86_64-linux"; }) lib pkgs;
        config = { programs.genote.package = forEachSystem.x86_64-linux.packages.genote; };
      };

      games-nixos = import ./modules/games/heroic.nix {
        inherit (import nixpkgs { system = "x86_64-linux"; }) lib pkgs;
        config = { };
      };

      games-hm = import ./modules/games/heroic-hm.nix {
        inherit (import nixpkgs { system = "x86_64-linux"; }) lib pkgs;
        config = { };
      };
    in
    forEachSystem // {
      homeManagerModules.default = hmModule;
      homeManagerModules.heroic = games-hm;
      nixosModules.heroic = games-nixos;
    };
}