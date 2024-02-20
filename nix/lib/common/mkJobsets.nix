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

        echo "hello" > /dev/stderr
        raw="$(curl -v \
          --cacert ${utils.pkgs.${system}.cacert}/etc/ssl/certs/ca-bundle.crt \
          -H "Accept: application/json" \
          -H "Authorization: ${authorizationKeyword} $token" \
          https://${api}/repos/${owner}/${repo}/branches >/dev/stderr)"
        echo "raw: $raw" > /dev/stderr
        echo "$raw" | jq '.
            | map({ (.name): {
                "url": ("${urlPrefix}" + .name),
                "flake": ${utils.lib.boolToString flake}
              }})
            | add'
      '';
    };
}
