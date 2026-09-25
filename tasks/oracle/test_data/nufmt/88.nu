# https://github.com/nushell/nufmt/pull/88

def foo [cls: closure] { }
def pick [path: cell-path] { }
pick a.b
try { ls } catch {|err| print $err }
