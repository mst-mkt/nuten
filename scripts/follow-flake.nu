# Makes the versions that must match nix/dev/flake.lock follow it.

const ROOT = path self ..

def main []: nothing -> nothing {
    cd $ROOT

    follow-vite-plus
    follow-rust
}

def follow-vite-plus []: nothing -> nothing {
    let version = ^nix eval --raw --no-update-lock-file --inputs-from ./nix/dev nix-vite-plus#vp.version

    ^nix shell --inputs-from ./nix/dev nixpkgs#pnpm_11 --command pnpm pkg set --config.runtime-on-fail=ignore $"devDependencies.vite-plus=($version)"
    ^nix develop --no-update-lock-file ./nix/dev --command pnpm install --lockfile-only
}

def follow-rust []: nothing -> nothing {
    let version = ^nix shell --inputs-from ./nix/dev fenix#stable.rustc --command rustc --version --verbose
        | lines
        | parse "{key}: {value}"
        | where key == release
        | get 0.value

    { toolchain: { channel: $version } } | to toml | save --force rust-toolchain.toml
}
