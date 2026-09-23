ls | each {|x, y| $x }
ls | each {|acc, x: int = 0| $acc }
ls | each {|a, b| $a }
ls | each {|x, y| $x }
ls | each {|x: int, y| $x }
ls | each {|x # c
| $x }
ls | each {|x, # c
y| $x }
ls | each {|x: record<a: int, b: string>, ...rest: list<int>| $x }
ls | each {|--flag(-f), --other(-o): int = 1| $flag }
ls | each {|x (-f)| $x }
ls | each {|x| $x } {|x| $x } {|x:| $x } {|x =| $x }
ls | each {| |} {||} {|x|}
ls | each {|x o> y| $x }
ls | each {|aaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccc, dddddddddddddddd|
    $aaaaaaaaaaaaaaaaaaaa
}
ls | each {|x # c
, y| $x }
ls | each {|o> x| $x }
ls | each {|x e> | $x } {|x = e> | $x } {|x: int err> | $x }
ls | each {|: int, y| $y } {|= x y| $y } {|(-f) y| $y }
