ls e>| lines
^ls e>| lines
| length
open data.json e>| get items
| where status == active
| sort-by created_at o+e>| select name email
| first 10
