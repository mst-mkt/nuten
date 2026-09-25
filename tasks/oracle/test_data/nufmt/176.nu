# https://github.com/nushell/nufmt/issues/176

def is-wayland [] { $env.WAYLAND_DISPLAY? != null }
if (is-wayland) { 1 } else if (is-wayland) { 2 }
