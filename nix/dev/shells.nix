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
  codium-headless = pkgs.writeShellScript "codium-headless" ''
    unset WAYLAND_DISPLAY
    exec ${pkgs.xvfb-run}/bin/xvfb-run --auto-servernum \
      ${pkgs.vscodium}/lib/vscode/codium --ozone-platform=x11 "$@"
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

  vscode = pkgs.mkShell {
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
