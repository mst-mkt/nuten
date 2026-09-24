match $x { $y if $y > 1 and not $z => a, $y if ($y|length) => b, _ if true => c }
match $x { 1 | 2 if $z in [1, 2] => b }
match $x { a | b | c if $y > 1 => a }
