# https://github.com/nushell/nufmt/issues/143

def errors [err] { $err }
def f [err] {
    if (errors $err) == "nu::shell::" {
      error make $err
    }
}
