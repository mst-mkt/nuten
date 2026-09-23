ls | where size > 10kb
where a.b == 1 and $x in [1, 2]
where {a: 1}
where {|x| $x > 1}
where {a: 1} == {a: 1}
where -h -- -1 == 1
where --help
where if true {true} else {false}
where 1 == match $x {_ => true}
where not $x
where a == 1 o> x
where {}
(where a ==
  1)
where { # c
}
where -- {a: 1}
where if true {true} else {false} --
