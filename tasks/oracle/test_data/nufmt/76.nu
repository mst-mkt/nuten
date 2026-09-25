# https://github.com/nushell/nufmt/issues/76

def pretty [] { ($"`(ansi reset)($in)`") }
def greet [] {
    ($"hello "
    + $"($in)")
}
