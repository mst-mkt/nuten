# https://github.com/nushell/nufmt/issues/139

def f [msg_type] {
    match $msg_type {
      $type if $type == 1 => { }
    }
}
