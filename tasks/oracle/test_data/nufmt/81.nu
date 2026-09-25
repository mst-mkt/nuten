# https://github.com/nushell/nufmt/issues/81

def f1 [] { [] }
def f2 [foo: int@f1 = 1] { }
