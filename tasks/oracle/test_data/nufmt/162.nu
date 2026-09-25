# https://github.com/nushell/nufmt/issues/162

def f [value] {
    let has_value: bool = ($value == null or $value == "") and ($value | is-not-empty)
}
