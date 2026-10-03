{
  pkgs,
  toolchain,
  treefmt,
  gitHooks,
  corpus,
  nutest,
  nuten,
}:
let
  xvfb-run = "${pkgs.xvfb-run}/bin/xvfb-run";
  codium = "${pkgs.vscodium}/lib/vscode/codium";
  codium-headless = pkgs.writers.writeNu "codium-headless" ''
    def --wrapped main [...arguments] {
        hide-env --ignore-errors WAYLAND_DISPLAY
        exec ${xvfb-run} --auto-servernum ${codium} --ozone-platform=x11 ...$arguments
    }
  '';
in
{
  default = pkgs.mkShell {
    packages = [
      toolchain.nushell
      toolchain.rust
      toolchain.nodejs
      toolchain.pnpm
      toolchain.vite-plus
      pkgs.cargo-insta
      pkgs.cargo-nextest
      treefmt.config.build.wrapper
      nuten

      pkgs.hyperfine
      pkgs.time
    ]
    ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
      pkgs.valgrind # Valgrind is broken on darwin
    ];

    env = {
      NU_LIB_DIRS = "${nutest}";
      NUTEN_CORPUS = corpus;
    };
    inherit (gitHooks) shellHook;
  };

  vscode-test = pkgs.mkShell {
    packages = [
      toolchain.rust
      toolchain.nodejs
      toolchain.pnpm
      toolchain.vite-plus
    ];

    env = pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux { NUTEN_VSCODE = codium-headless; };
  };

  fuzz = pkgs.mkShell {
    packages = [
      toolchain.nightlyRust
      pkgs.cargo-fuzz
    ];

    env = {
      LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
        pkgs.stdenv.cc.cc.lib # libFuzzer needs libstdc++
      ];
    };
  };
}
