{
  sources ? import ./sources.nix,
  system ? builtins.currentSystem or "unknown-system",
  pkgs ? import ./nixpkgs.nix {inherit sources system;},
  rust ? import ./rust.nix {inherit sources system;},
}: let
  rustPackages = builtins.attrValues {
    inherit (rust) rustToolchain;
    inherit (pkgs) rustfmt rust-analyzer pkg-config;
  };
in rec {
  default = server;

  server = pkgs.mkShell {
    name = "typhon-devshell";
    packages =
      rustPackages
      ++ builtins.attrValues {
        inherit (pkgs.nixVersions) nix_2_18;
        inherit
          (pkgs)
          bubblewrap
          diesel-cli
          sqlite
          ;
      };
    DATABASE_URL = "sqlite:typhon.sqlite";
    TYPHON_FLAKE = ../typhon-flake;
  };

  webapp = pkgs.mkShell {
    name = "typhon-webapp-devshell";
    packages =
      [ (pkgs.writeShellScriptBin "rustfmt" ''
           ${pkgs.leptosfmt}/bin/leptosfmt "$@"
           ${pkgs.rustfmt}/bin/rustfmt "$@"
        '') ]
      ++ rustPackages
      ++ builtins.attrValues {
        inherit (pkgs.nodePackages) sass;
        inherit (pkgs) trunk nodejs;
      }
    ;
    RUSTFLAGS = "--cfg=web_sys_unstable_apis";
  };

  types = pkgs.mkShell {
    name = "typhon-types-devshell";
    packages = rustPackages;
  };

  doc = pkgs.mkShell {
    name = "typhon-doc-devshell";
    packages = [pkgs.mdbook];
  };

  test-api = pkgs.mkShell {
    name = "typhon-test-api-devshell";
    packages = rustPackages ++ [pkgs.openssl];
  };
}
