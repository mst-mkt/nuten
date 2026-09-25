# https://github.com/nushell/nufmt/issues/82

def in-lang [ln] { $in | items {|k v| {$k: ($v | get $ln | default ($v | get 'en')) } } | reduce -f {} {|it acc| $acc | merge $it } }
