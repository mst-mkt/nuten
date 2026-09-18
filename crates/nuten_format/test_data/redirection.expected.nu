ls o> out.txt
ls err>> err.log
ls o+e> "all logs.txt"
ls o>> out.txt e> err.log | length
^echo a out+err>> out.txt b
ls o> f a b | first
open data.json
| get items
| where status == active
| sort-by created_at
| select name o> out.txt e> err.log
ls o> a.txt e>| lines
ls o> [a b]
ls o> { a: 1 }
