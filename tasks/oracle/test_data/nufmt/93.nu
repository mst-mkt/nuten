# https://github.com/nushell/nufmt/issues/93

def f [items] {
    let result = $items | from json | if ($in | default [] | where value == "ERR" | is-empty) {
        $in
    } else {
        null
    }
}
