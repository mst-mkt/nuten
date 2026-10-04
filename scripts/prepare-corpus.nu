# Assembles the corpus as nix/dev/corpus.nix does, at the revisions nix/dev/flake.lock pins, for a system without nix.

const LOCK = path self ../nix/dev/flake.lock

const SOURCES = [[name, input, dir];
    [nu-std, nushell, crates/nu-std]
    [nu_scripts, corpus-nu_scripts, .]
    [nupm, corpus-nupm, .]
    [tree-sitter-nu, corpus-tree-sitter-nu, test/corpus]
    [kurokirasama-nushell_scripts, corpus-kurokirasama-nushell_scripts, .]
]

# Writes the corpus into out, which must not exist yet.
def main [out: path]: nothing -> nothing {
    let nodes = open --raw $LOCK | from json | get nodes
    mkdir $out

    for source in $SOURCES {
        let locked = $nodes | get ($nodes.root.inputs | get $source.input) | get locked
        let repo = checkout $locked.owner $locked.repo $locked.rev
        cp -r ($repo | path join $source.dir) ($out | path join $source.name)
        rm -r $repo
    }

    cd $out
    glob **/* --no-dir
    | where { ($in | path parse | get extension) not-in [nu, nuon] }
    | each { rm $in }
    | ignore
}

# A checkout of the repository at rev, without its .git.
def checkout [owner: string, repo: string, rev: string]: nothing -> path {
    let dir = mktemp --directory
    let options = [-C, $dir, -c, core.autocrlf=false]

    ^git ...$options init --quiet
    ^git ...$options fetch --quiet --depth 1 $"https://github.com/($owner)/($repo)" $rev
    ^git ...$options checkout --quiet FETCH_HEAD

    rm -r ($dir | path join .git)
    $dir
}
