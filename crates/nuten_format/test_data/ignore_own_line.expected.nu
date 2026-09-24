# nuten-ignore
let table = [
  [name   size];
  [a      1   ]
  [bb     22  ]
]

$env.config = {
    # nuten-ignore
    keybindings: [
        { name: a   modifier: control }
        { name: bb  modifier: alt }
    ]
    show_banner: false
}

let flags = [
    # nuten-ignore
    { output:   json }
    { lines: $n }
]

def main [] {
    # nuten-ignore
    if  $a  {  1  }  else  {  2  }
    if $a {
        1
    } else {
        2
    }
}

# nuten-ignore laid out as a table
# and the comments after it stay above
let widths = {
  a:    1
}

# nuten-ignore

let after_blank_line = [1
  2]

#nuten-ignore
let without_space = [1
  2]

# nuten-ignored
let other_word = [1 2]

match $x {
    # nuten-ignore
    1   =>   'one'
    2 => 'two'
}

ls
# nuten-ignore
| where   size > 1
| get name

ls |
# nuten-ignore
where   size > 1

def f [
    # nuten-ignore
    --flag   (-f) # description
    --other(-o)
] {}

(
    ls
    # nuten-ignore
    | where ($it.size   +   1) > 2
)

(
    ls
    # nuten-ignore
    | $x   =   1
)

(
    if $a
    # nuten-ignore
    {  1  } else {
        2
    }
)

(
    def h []
    # nuten-ignore
    {  1  }
)

if $a {
    if $b {
        # nuten-ignore
        let text = "a
  b"  |  str trim
    }
}

def g [] {
    ls
    # nuten-ignore
}

(
    ls
    # nuten-ignore
    | let x   =   a
    b
)

let y = # c
# nuten-ignore
| length   a
