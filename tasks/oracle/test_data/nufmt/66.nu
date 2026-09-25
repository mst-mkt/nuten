# https://github.com/nushell/nufmt/issues/66

def greet [name] {
  ["hello" $name]
}

greet "world"
