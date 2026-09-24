open data.json
| get items
| where status == active
| sort-by created_at
| select name o> out.txt e> err.log
