if true {
  ls # e
}
match 1 {1 => 2 # m}
match {a: 1} {{a: $x # p} => $x}
do { 1 # i}
ls | where {|x| $x # w}
