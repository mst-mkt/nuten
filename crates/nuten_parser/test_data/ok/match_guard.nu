match $x { $y if $y > 1 and not $z => a, $y if ($y|length) => b, _ if true => c }
match $x { $y if
  $y >
  # c
  1
  => a }
match $x { $y if if true {1} else {2} => a, $y if $y | z => b, $y if 1 + if true {1} else {2} => c }
match $x { 1 | 2 if $z in [1, 2] => b }
match $x { _ if => a, $y if match $y { _ => true } => b }
match $x { a | b | c if $y > 1 => a }
match 1 { _ if match ls $a => 1 }
