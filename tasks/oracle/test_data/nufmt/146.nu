# https://github.com/nushell/nufmt/issues/146

def f [thing] {
    if true {
      # noop
    } else {
      do $thing
    }
}
