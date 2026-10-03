{
  lib,
  stdenv,
  writers,
  coreutils,
  hyperfine,
  time,
  git,
  valgrind,
  util-linux,
  toolchain,
  nutest,
}:
let
  path = lib.makeBinPath (
    [
      toolchain.nushell
      toolchain.rust
      stdenv.cc
      coreutils
      hyperfine
      time
      git
    ]
    ++ lib.optionals stdenv.hostPlatform.isLinux [
      valgrind
      util-linux
    ]
  );
in
writers.makeScriptWriter {
  interpreter = "${lib.getExe toolchain.nushell} --no-config-file";
  makeWrapperArgs = [
    "--set"
    "NU_LIB_DIRS"
    "${nutest}"
    "--prefix"
    "PATH"
    ":"
    path
  ];
} "/bin/nuten-test-nu" ../../scripts/test-nu.nu
