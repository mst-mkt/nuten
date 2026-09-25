# https://github.com/nushell/nufmt/issues/228

const my_const = '/and/another/path'
def with_flag [--file: path = $nu.history-path] {}
def cmd [--var: any = $my_const] {}
