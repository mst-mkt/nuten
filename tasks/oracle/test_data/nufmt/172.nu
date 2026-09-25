# https://github.com/nushell/nufmt/issues/172

def has [cmd: string] { which $cmd | is-not-empty }
if not (has systemctl) { 1 }
