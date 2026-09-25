# https://github.com/nushell/nufmt/issues/111

def bar [a b] { }
def foo []: nothing -> nothing {
    let x = (bar
      4 # 44
      5 # 55
    )
}
