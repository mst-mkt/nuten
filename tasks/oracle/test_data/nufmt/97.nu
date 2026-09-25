# https://github.com/nushell/nufmt/issues/97

def start_zellij [] {
  if ($env.ZELLIJ_AUTO_ATTACH? | default "false") == "true" {
    zellij attach -c
  } else {
    zellij
  }
}
