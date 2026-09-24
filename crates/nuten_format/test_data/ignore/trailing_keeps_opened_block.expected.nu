$env.config = { # nuten-ignore
  a:   1
}

if $a {
    1
} else   { # nuten-ignore
  2
}

def main [] {
    if $a {
        1
    } else { # nuten-ignore
    let text = "a
b"
  }
}

def h [] { # nuten-ignore
  1
}

let nested = [
    [1  2] [ # nuten-ignore
    3  4 # nuten-ignore
  ]
]
