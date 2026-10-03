{
  pkgs,
  inputs,
  nushell,
}:
pkgs.runCommand "nuten-corpus" { } ''
  mkdir $out
  cp -r ${nushell.src}/crates/nu-std $out/nu-std
  cp -r ${inputs.corpus-nu_scripts} $out/nu_scripts
  cp -r ${inputs.corpus-nupm} $out/nupm
  cp -r ${inputs.corpus-tree-sitter-nu}/test/corpus $out/tree-sitter-nu
  cp -r ${inputs.corpus-kurokirasama-nushell_scripts} $out/kurokirasama-nushell_scripts
''
