# Makes the versions that must match nix/dev/flake.lock follow it.

const ROOT = path self ..

def main []: nothing -> nothing {
    cd $ROOT

    follow-vite-plus
    follow-rust
}

def follow-vite-plus []: nothing -> nothing {
    let version = ^nix eval --raw --no-update-lock-file --inputs-from ./nix/dev nix-vite-plus#vp.version
    let catalog = { vite: $"npm:@voidzero-dev/vite-plus-core@($version)", vite-plus: $version }

    open --raw pnpm-workspace.yaml | follow-catalog $catalog | save --force pnpm-workspace.yaml
    ^nix develop --no-update-lock-file ./nix/dev --command pnpm install --lockfile-only
}

def follow-catalog [catalog: record]: string -> string {
    let workspace = $in
    let followed = $catalog
        | transpose key value
        | reduce --fold $workspace {|entry, text|
              $text
              | str replace --regex $"\(?m\)^  ($entry.key): .*$" $"  ($entry.key): ($entry.value)"
          }

    if ($followed | from yaml) != ($workspace | from yaml | update catalog { merge $catalog }) {
        error make {
            msg: "the catalog in pnpm-workspace.yaml is not in the shape follow-flake.nu rewrites"
        }
    }
    $followed
}

def follow-rust []: nothing -> nothing {
    let version = ^nix shell --inputs-from ./nix/dev fenix#stable.rustc --command rustc --version --verbose
        | lines
        | parse "{key}: {value}"
        | where key == release
        | get 0.value

    { toolchain: { channel: $version } } | to toml | save --force rust-toolchain.toml
}
