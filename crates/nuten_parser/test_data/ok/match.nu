match $x { 1 => a, _ => b }
match $x {
  1 => { a }
  _ => b
}
match ($x | length) { 0 => null, $n => { print $n } }
match $x { [a b] => 1, {c: $d} => 2, 1..3 => 3, a | b => 4, $y if $y > 1 => 5, _ => 6 }
let v = match $x { 1 => a, _ => b }
match $x { 1 => a } | length
match $x {}
match $x { # c
  1 => a # d
  # e
  _ => b,
}
match $x { 1 => a; _ => b }
match $x { 1 => a b }
match $x { => a }
match $x { 1 =>, _ => b }
match $x { 1 => a | b }
match $x { 1 => a || b }
match $x
match $x { 1 => a } extra
match $x { 1
=> a }
match $x { 1 =>
a }
match $x { 1 => {|y| b } }
match $x { "a b" => 1, _ => {a: 1} }
match $x { 1 => a,, _ => b }
match $x { 1, 2 => a }
match $x { 1 # c
=> a }
match $x { , 1 => a }
match $x { ,, }
match $x { 1 => # c
a }
match $x { "=>" => a, $y if $y == "=>" => b }
match $x { 1=>a }
match a b { _ => 1 }
match $x { 1 => a } o> f.txt
ls | match $in { 1 => a }
match $x { 1, => a }
match $x { a: 1 }
