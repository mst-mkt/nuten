match $x {
    (1)..3 => a
    (1 + 2) => b
    0x[ff 00] | ..5 => c
    (1) if $y => d
    _ => e
}
