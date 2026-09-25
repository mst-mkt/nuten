# https://github.com/nushell/nufmt/issues/224

def main [] {
    let value = $env."KEY(with-parens)"
    $value
}
