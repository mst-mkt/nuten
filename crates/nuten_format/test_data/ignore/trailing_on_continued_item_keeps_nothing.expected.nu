ls | each {|x|
    $x.name # nuten-ignore
}

match 1 {
    1
    => 2 # nuten-ignore
    _ => 3
}

let leading_colon = {
    a
    : 1 # nuten-ignore
}

def leading_eq [
    x: int
    = 3 # nuten-ignore
] {}
