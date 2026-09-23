match $x { 1..3 => a, ..5 | 7.. => b, (1 + 2) => c, ( 1 )..3 => d, 0x[ff] => e, "a" | 'b' | r#'c'# => f, _ => g }
match $x { 1..a => a, ...$y => b, -1 => c, 1kb => d, null => e, (1).a => f }
match $x { $y => a, [$y] => b, {a: $y} => c }
match $x { 1 2 => a, ( 1 ) if $y => c }
match $x {
  (1)
  | (2)
  => a
}
match $x { 1 | 2 | 3..5 => a }
