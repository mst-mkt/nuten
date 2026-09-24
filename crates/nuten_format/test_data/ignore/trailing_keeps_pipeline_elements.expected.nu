ls
| where  size > 1  | get  name # nuten-ignore
| first 2

(
    ls
    | where  $it.size  >  1 # nuten-ignore
)

ls
| $x   =   1 # nuten-ignore
