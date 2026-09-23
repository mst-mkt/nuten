match $x { [] => a, [1, 2..3, (1)] => b, [$y, ..$rest] => c, [a b ..] => d, [[1] [..3]] => e }
match $x { [a # c
  b
] => a, [..3 ..=4] => b, [...$y] => c, [$y {a: 1}] => d }
match $x { [a | b] => a, [a; b] => b, [a o> b] => c, [a = b] => d, [.. a] => e, [..$r a] => f }
match $x { [a,#c
] => a, [a] | [b] if $c => b }
match $x { a | b | [c, d] => a }
