# https://github.com/nushell/nufmt/issues/87

def "complete cargo packages" [] { ["nuten"] }
export extern "cargo build" [
  --package (-p): string@"complete cargo packages"
  --release (-r)
  -Z: any
  -h
  --help
  --jobs (-j): number
]
