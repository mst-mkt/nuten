[...$x ...(ls | get name) ...[1 2]]
{ ...$x a: 1 ...(f) }
^echo ...$args [...$x]
print ...$args
[...(ls | where size > 1kb | sort-by modified | reverse | first 10 | get name | str join ", ")]
[
    ...(
        ls
        | where size > 1kb
        | sort-by modified
        | reverse
        | first 10
        | get name
        | str join ", "
        | str trim
    )
]
^echo ...$x ...(ls) ...[1 2]
