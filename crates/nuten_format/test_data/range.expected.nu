$x.a..(ls | length)
(1)..<$y.b
$x..(1)..(2)
print $x..$y ($n + 1)..$m
[$x.. (1)..3]
$x...
(
    ls
).a..3
$x..(
    ls
    | where size > 1kb
    | sort-by modified
    | reverse
    | first 10
    | get name
    | str join ", "
    | length
)
