{
  sources ? import ../sources.nix,
  system ? builtins.currentSystem or "unknown-system",
  pkgs ? import ../nixpkgs.nix {inherit sources system;},
  rust ? import ../rust.nix {inherit sources system;},
}: let
  inherit (pkgs) lib;

  inherit (rust) craneLib;

  src = lib.sourceByRegex ../.. [
    "Cargo.toml"
    "Cargo.lock"
    "typhon.*"
  ];

  cargoToml = ../../typhon-webapp/Cargo.toml;

  cargoLock = ../../Cargo.lock;

  RUSTFLAGS = "--cfg=web_sys_unstable_apis";

  cargoArtifacts = craneLib.buildDepsOnly {
    inherit src cargoToml RUSTFLAGS;
    cargoExtraArgs = "-p typhon-webapp --target wasm32-unknown-unknown";
    doCheck = false;
  };

  nodeDependencies =
    (pkgs.callPackage webapp/npm-nix {}).nodeDependencies;

  trunkPackage = craneLib.buildTrunkPackage {
    inherit
      src
      cargoToml
      cargoArtifacts
      RUSTFLAGS
      ;
    trunkIndexPath = "typhon-webapp/index.html";
    preBuild = ''
      ln -s ${nodeDependencies}/lib/node_modules typhon-webapp/node_modules
      echo 'build.public_url = "WEBROOT"' >> Trunk.toml
    '';

    doNotRemoveReferencesToVendorDir = true;
  };

  cleanWasm = pkgs.stdenv.mkDerivation {
    name = "typhon-webapp-clean-wasm";
    src = trunkPackage;
    nativeBuildInputs = [craneLib.removeReferencesToVendoredSourcesHook];
    cargoVendorDir = craneLib.vendorCargoDeps {inherit cargoLock;};
    installPhase = ''
      runHook preInstall
      mkdir -p $out
      cp *.wasm $out
      runHook postInstall
    '';
  };

  crateName = craneLib.crateNameFromCargoToml {inherit cargoToml;};

  tarball = pkgs.stdenv.mkDerivation {
    name = "source.tar.gz";
    src = ../..;
    buildPhase = ''
      tar -czf $out \
        --sort=name \
        --transform 's/^/typhon\//' \
        .
    '';
  };
in
  pkgs.callPackage (
    {
      webroot ? "",
      api_url ? "http://127.0.0.1:8000/api",
    }: let
      settings = builtins.toJSON {inherit api_url;};
    in
      pkgs.stdenv.mkDerivation {
        inherit (crateName) pname version;
        src = trunkPackage;
        buildPhase = ''
          cp ${cleanWasm}/* .
          substituteInPlace ./index.html --replace \
              'WEBROOT' \
              '${webroot}/'
          substituteInPlace ./index.html --replace \
              '<script type="application/json" id="settings">null</script>' \
              '<script type="application/json" id="settings">${settings}</script>'
          cp ${tarball} source.tar.gz
        '';
        installPhase = ''
          mkdir -p $out${webroot}
          cp -r * $out${webroot}
        '';
      }
  ) {}
