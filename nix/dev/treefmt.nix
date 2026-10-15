{
  pkgs,
  inputs,
  toolchain,
}:
inputs.treefmt-nix.lib.evalModule pkgs {
  imports = [ inputs.nuten.treefmtModule ];

  projectRootFile = "Cargo.toml";

  programs = {
    nixfmt = {
      enable = true;
      strict = true;
    };
    deadnix = {
      enable = true;
    };
    statix = {
      enable = true;
    };
    rustfmt = {
      enable = true;
      package = toolchain.rust;
    };
    taplo = {
      enable = true;
    };
    oxfmt = {
      enable = true;
      excludes = [ "pnpm-lock.yaml" ];
    };
    nuten = {
      enable = true;
      excludes = [
        "crates/*/test_data/**"
        "tasks/oracle/test_data/**"
        "playground/src/lib/sample.nu"
      ];
    };
  };

  settings.formatter = {
    oxfmt.command = "${toolchain.vite-plus}/node_modules/oxfmt/bin/oxfmt";
  };
}
