{
  pkgs,
  inputs,
  toolchain,
  treefmt,
}:
let
  commitTypes = [
    "feat"
    "add"
    "fix"
    "refactor"
    "perf"
    "style"
    "test"
    "docs"
    "ci"
    "chore"
  ];
  commitScopes = [
    "cli"
    "syntax"
    "parser"
    "doc"
    "format"
    "config"
    "line-index"
    "nix"
    "oracle"
    "benchmark"
    "lsp"
    "vscode"
    "zed"
    "dprint"
    "assets"
  ];
  convcoConfig = (pkgs.formats.yaml { }).generate "convco.yaml" {
    types = map (type: {
      inherit type;
      section = type;
    }) commitTypes;
    scopeRegex = "^(${pkgs.lib.concatStringsSep "|" commitScopes})$";
  };
in
inputs.git-hooks.lib.${pkgs.stdenv.hostPlatform.system}.run {
  src = ../..;

  hooks = {
    treefmt = {
      enable = true;
      package = treefmt.config.build.wrapper;
    };

    clippy = {
      enable = true;
      packageOverrides = {
        cargo = toolchain.rust;
        clippy = toolchain.rust;
      };
      settings = {
        denyWarnings = true;
        extraArgs = "--all-targets";
      };
    };

    typos = {
      enable = true;
    };

    check-merge-conflicts = {
      enable = true;
    };

    convco = {
      enable = true;
      settings.configPath = convcoConfig;
    };
  };
}
