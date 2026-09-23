match $x {
    (1)..3 => a
    (1 + 2) => b
    0x[ff 00] | ..5 => c
    (1) if $y => d
    _ => e
}
match $x {
    (1) | (2) => a
    1 2 => b
}
match $x {
    $y if ( $y|length )>1 => a
    $y if $y in [1 2] and not $z => b
    $y if $y | z => c
}
match $x {
    $y if $y >
    # c
    1 => a
}
match $x {
    $y if 1 + if true { 1 } else { 2 } => a
    $y if (ls | each {|x, y| $x }) => b
}
