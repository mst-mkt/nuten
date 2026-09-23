if $a {
    print a
}
if $a {
    print a
} else {
    print b
}
if $a {
    print a
} else {
    null
}
if $a {
    1
} else if $b {
    2
} else {
    3
}
if $a > 1 and $b {} else {
    print b
}
let x = if $a { 1 } else { 2 }
let x = if $a { 1 } else { 2 }
let x = if $a {
    print a; 1
} else {
    2
}
let x = if $a { 1 } else if $b { 2 } else { 3 }
let long = if $aaaaaaaaaaaaaaaaaaaaaaaaaa {
    bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
} else {
    cccccccccccccccccccccccccccc
}
ls | each {|f| if $f.size > 1kb { big } else { small } }
ls | each {|f|
    if $f.size > 1kb {
        print big
    }
    $f
}
def f [] {
    if $x { print a }
}
ls | if $a { 1 } else 2 | length
if $a { # c
    1
} else {
    2
}
if $a {
    1
} else {
    2
} o> out.txt
if $x { a } else { b } { c }
if $x {
    1
}
else {  2  }
do {
    if $a {
        1
    } else {
        2
    };
}
do { if $a { 1 } else { 2 } }
let x = if $a { 1 } else {|y| $y }
let x = if $a {

    | length
} else {
    2
}
let x = if $a { # c
} else {
    2
}
ls
| if $a {
      print a; 1
  } else {
      2
  }
| length
if $x {
    break
} # c
ls | each {|x|
    if $x.a {
        print a
    }; $x
}
for it in $items {
    if $a {
        $s ++= [1]
    }
    if $b {
        $s ++= [2]
    }
}
do {
    if $a { 1 } else { 2 } # c
}
if true {} else {
    ...$x, a: 1
}
if true {} else [1 2]
if true {} else { a: 1 }
if -- $x == 1 {
    1
}
if -h {
    1
}
if match 1 {
    _ => true
} {
    1
}
if $a {
    1
} else foo {1}
if $a {
    1
} else 1 + 2
if $a {
    1
} else foo {1} o> f
if $a {
    1
} else match $x {
    _ => 2
}
let y = if $a { 1 } else print [1,  2]
if $a {
    1
} else @foo {1}
if $a {
    1
} else if $b {
    2
} else foo {1}
if $a { 1 } else $a --help
if $a { 1 } else (1) --help
if $a { 1 } else r#'x'# --help
