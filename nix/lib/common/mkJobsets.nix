utils: lib: {
  mkJobsets = {
    api,
    authorizationKeyword,
    flake,
    owner,
    repo,
    tokenName,
    urlPrefix,
  }:
    lib.builders.mkActionScript {
      mkPath = system: let
        pkgs = utils.pkgs.${system};
      in [
        pkgs.curl
        pkgs.jq
      ];
      mkScript = system: ''
        input=$(cat)

        token=$(echo "$input" | jq -r '.secrets.${tokenName}')

        echo "hello"
        raw="$(curl -v \
          --cacert ${utils.pkgs.${system}.cacert}/etc/ssl/certs/ca-bundle.crt \
          -H "Accept: application/json" \
          -H "Authorization: ${authorizationKeyword} $token" \
          https://${api}/repos/${owner}/${repo}/branches)"
        echo "raw: $raw"
        echo "$raw" | jq '.
            | map({ (.name): {
                "url": ("${urlPrefix}" + .name),
                "flake": ${utils.lib.boolToString flake}
              }})
            | add'
      '';
    };
}
