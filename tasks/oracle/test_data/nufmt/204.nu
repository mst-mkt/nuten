# https://github.com/nushell/nufmt/issues/204

const my_const = '/and/another/path'
def with_nu_var [file: path = $nu.history-path] { print $file }
def with_const [file: path = $my_const] { print $file }
