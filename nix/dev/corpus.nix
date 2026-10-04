{
  pkgs,
  inputs,
  nushell,
}:
let
  sources = {
    nu-std = "${nushell.src}/crates/nu-std";
    nu_scripts = inputs.corpus-nu_scripts;
    nupm = inputs.corpus-nupm;
    tree-sitter-nu = "${inputs.corpus-tree-sitter-nu}/test/corpus";
    kurokirasama-nushell_scripts = inputs.corpus-kurokirasama-nushell_scripts;
  };
  nuten-corpus = ''
    mkdir $out
    ${pkgs.lib.concatMapAttrsStringSep "\n" (name: source: "cp -r ${source} $out/${name}") sources}
    chmod -R u+w $out
    find $out -type f ! -name '*.nu' ! -name '*.nuon' -delete
  '';
in
pkgs.runCommand "nuten-corpus" { } nuten-corpus
