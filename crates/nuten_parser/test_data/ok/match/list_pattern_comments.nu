match $x { [a # c
  b
] => a, [..3 ..=4] => b, [...$y] => c, [$y {a: 1}] => d }
match $x { [a,#c
] => a, [a] | [b] if $c => b }
