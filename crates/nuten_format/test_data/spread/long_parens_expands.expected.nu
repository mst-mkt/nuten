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
