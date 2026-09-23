do {|x, y: int = 2, ...rest| $x}
do {|x: record<a: int, b: list<int>>| $x}
do {|--flag (-f), --other(-o): int| $flag}
do {|x:| $x} {|x =| $x} {|x: int =| $x}
do {|x,| $x} {|,x| $x}
do {|x
: int
, y| $x}
do {|x # c
| $x}
do {| x |}
do {|x (-f)| $x}
do {|x,,y| $x} {|x: record<a: int|b>| $x}
do {|o> x| $x} {|x o> y| $x} {|x e> | $x}
do {|: int, y| $y} {|= x y| $y}
