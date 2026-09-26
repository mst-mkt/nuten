match $x {
    [a b ..$rest] => 1
    [] => 2
    [[1 2] (1)..3] => 3
    [a | b] => 4
    [a] | [b] => 5
}
match $x {
    a | b | [c d] => 1
}
