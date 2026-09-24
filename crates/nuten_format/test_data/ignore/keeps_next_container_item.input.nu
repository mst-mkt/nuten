$env.config = {
    # nuten-ignore
    keybindings: [
        { name: a   modifier: control }
        { name: bb  modifier: alt }
    ]
    show_banner:   false
}

let flags = [
  # nuten-ignore
  { output:   json }
  { lines:   $n }
]

match $x {
    # nuten-ignore
    1   =>   'one'
    2   =>   'two'
}

def f [
    # nuten-ignore
    --flag   (-f)   # description
    --other   (-o)
] {}
