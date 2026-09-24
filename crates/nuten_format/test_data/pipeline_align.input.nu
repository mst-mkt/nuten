$rows | each {|row|
  $row.name
} | str join
$rows | each {|row| $row.name } | str join
ls | each {|x|
  $x
}
ls
| where name in [
  a
  b
] | length
ls
| where {|x|
$x
} e>| each {|y|
$y
}
ls
# c
| each {|x|
$x
}
ls | # c
each {|x|
$x
}
ls
| each {|x|
  $x
  | each {|y|
    $y
  }
}
ls
| str join "a
b"
let x = ls
| each {|x|
$x
}
ls
| get {
a: 1
}
ls |
each {|x|
$x
}
ls
| a o+e>| each {|x|
$x
}
ls | # c
| each {|x|
$x
}
ls; | each {|x|
$x
}
echo {|| | each {|x|
$x
}}
ls; |
each {|x|
$x
}
