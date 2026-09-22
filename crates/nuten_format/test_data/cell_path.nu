$x.a?!.b
$x."a b".`c`
(ls  |  get 0).name
( ls ).name?
(
ls
).name
(ls # c
).name
print (  ls  ).a ( 1 + 2 ).b
$env.PATH  =  ( $env.PATH | append a ).0
[( ls ).a]
{a: ( ls ).b}
(ls | where size > 1kb | sort-by modified | reverse | first 10 | get name | str join ", " | str trim).0
print (ls | where size > 1kb | sort-by modified | reverse | first 10 | get name | str join ",").abcdefg
[1   2].0
[[a];[1]].a
{a:1}.a?
print {a:1}.a
if true { } else [ 1 ].0
[1] | get  $.0?
{alpha: 1, beta: 2, gamma: 3, delta: 4, epsilon: 5, zeta: 6, eta: 7, theta: 8, iota: 9, kappa: 10}.alphabetagamma
[alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau].0
