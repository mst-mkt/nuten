match $x {
    [
        a # c
        b
    ] => 1
    {
        a # c
        : 1
    } => 2
    $y if # c
    $y > 1 => 3
}
match $x {
    [a b] => 1
    { a: 1 } => 2
}
