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
