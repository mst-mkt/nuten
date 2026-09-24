ls
# nuten-ignore
| where   size > 1
| get name

ls |
# nuten-ignore
where   size > 1

(
    ls
    # nuten-ignore
    | where ($it.size   +   1) > 2
)
