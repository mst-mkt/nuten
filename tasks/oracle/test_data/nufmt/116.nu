# https://github.com/nushell/nufmt/issues/116

def foo []: bool -> bool {
  let yesno: bool = ($in | str trim) == "yes"
  $yesno
}
