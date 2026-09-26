match $x {
    1 => a
    _ => b
}
match ($x | length) {
    0 => null
    $n => { print $n; $n }
}
match $x {
    [a, b] => 1
    { c: $d } => 2
    1..3 => 3
    a | b => 4
    $y if $y > 1 => 5
    _ => 6
}
