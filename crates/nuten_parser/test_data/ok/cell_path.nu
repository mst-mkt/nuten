$x.a
$env.PATH.0
$x.a?
$x.a!
$x.a?!.b!?
$x.a.
$x.
$x.?.!.
$x."a b".'c'.`d e`
$x.r#'a'#.$'b'
$x.0x10.1_0.+1
$x.a-b.a:b.a#b
$x.#a
$x.a?#b
$x.a..
$x."a\n"
(ls).name
(ls | get 0).name?
(
ls
).name
((ls).a).b
print $x.a (ls).b
^echo $x.a (ls).b
ls o> $x.a
[$x.a (ls).b]
{a: $x.b}
if $x.a { } else $x.b
match $x.a { _ => $x.b }
ls | where $it.a > 1
$env.FOO = $x.a
$x.a + (ls).b
let y = $x.a
$"($x.a)"
[1 2].0
[[a]; [1]].a
{a: 1}.a?
[a;b].0
print [1 2].0 {a: 1}.a
^echo {a: 1}.b
if true { } else [1 2].0
match 1 { _ => {a: 1}.a }
loop {a: 1}.a
ls | where {a: 1}.a
{}.a
$.a?
$.
$.?.b!
get $.a.0
(1..2).0
(ls).a..
(a)b.c
$x.=.o>
$x.a!.b
foo $x.a (ls).a
ls | where (ls).a
[1..2].0
{a: 1..2}.a
[1 2].0..
{a: 1}.a..
foo $.a
^echo $.a
if true { } else $.a
get $.a..
