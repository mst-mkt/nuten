# https://github.com/nushell/nufmt/issues/107

def foo []: nothing -> nothing {
  for h: int in [1 2 3] {
    print $h
  }
}
