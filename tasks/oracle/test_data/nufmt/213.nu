# https://github.com/nushell/nufmt/issues/213

$env.config = $env.config
# | upsert edit_mode vi
| upsert show_banner false
