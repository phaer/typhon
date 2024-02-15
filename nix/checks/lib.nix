{
  inputs ? import ../inputs.nix,
  system ? builtins.currentSystem or "unknown-system",
  pkgs ? import ../nixpkgs.nix {inherit inputs system;},
  lib ? import ../lib {inherit inputs;},
}: let
  owner = "typhon-ci";
  repo = "typhon";
  url = "github:typhon-ci/typhon";
  typhonUrl = "https://etna.typhon-ci.org";
  secrets = ./lib.nix;
  all = {
    github =
      (lib.github.mkProject {
        inherit owner repo secrets typhonUrl;
      })
      .actions;
    gitea =
      (lib.gitea.mkProject {
        inherit owner repo secrets typhonUrl;
        instance = "codeberg.org";
      })
      .actions;
    dummy =
      (lib.dummy.mkProject {
        inherit url;
      })
      .actions;
    git = lib.git.mkJobsets {
      inherit url;
    };
    cachix = lib.cachix.mkPush {
      name = "typhon";
    };
  };
in
  pkgs.stdenv.mkDerivation {
    name = "lib-checks";
    buildInputs = map (x: x.${system}) (pkgs.lib.attrValues all);
    phases = ["installPhase"];
    installPhase = "touch $out";
  }
