(
ls
).a..3
$x..(ls | where size > 1kb | sort-by modified | reverse | first 10 | get name | str join ", " | length)
