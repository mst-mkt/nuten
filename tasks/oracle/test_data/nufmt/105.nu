# https://github.com/nushell/nufmt/issues/105

def foo [x: int] {
  match $x {
    1 => print "one"
    _ => print "other"
  }
}
