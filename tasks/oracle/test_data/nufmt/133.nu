# https://github.com/nushell/nufmt/issues/133

def base16 [a b] { $b }
export def colors [] {
  {
    warn: (base16 "base0A" "yellow") # warning
  }
}
