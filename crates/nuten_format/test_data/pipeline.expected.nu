ls | get name | first
open a.txt | lines | length
ls
| get name
| first
ls
| length
open data.json
| get items
| where status == active
| sort-by created_at
| select name email created_at
| first 10
open data.json | get items | where status == active | sort-by created_at | select name | first 10
ls e>| lines
ls | get name # a long comment at the end of a line does not count toward the width of the line, however long
ls | | first
ls
|
| first
^ls e>| lines
| length
open data.json e>| get items
| where status == active
| sort-by created_at o+e>| select name email
| first 10
