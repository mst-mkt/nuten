match $x { 1 => a, _ => b }
match $x {
  1 => { a }
  _ => {
    b
  }
}
match ($x | length) { 0 => null, $n => { print $n; $n } }
match $x { [a b] => 1, {c: $d} => 2, 1..3 => 3, a | b => 4, $y if $y > 1 => 5, _ => 6 }
let v = match $x { 1 => a, _ => b }
match $x {}
match $x { # c
  1 => a # d
  # e

  _ => b,
}
match $x { 1 => a; _ => b }
match $x { 1
=>
a }
def f [] { match $x { 1 => a } }
match $x { 1 => a } | length
match $x {
  1 => {

    a

  }
}
match $x { 1 => { match $y { 2 => b } } }
match $x { 1 => a,, _ => b }
match $x { , }
match $x {

  1 => a

}
match $x { 1 => {a: 1, b: 2}, _ => [1, 2] }
match 1 {1 => a,#c [
] => b
}
