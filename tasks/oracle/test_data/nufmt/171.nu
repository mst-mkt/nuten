# https://github.com/nushell/nufmt/issues/171

alias ez = eza --git --long --header
def ezt [level: int = 2] {
    ez --tree --level $level
}
