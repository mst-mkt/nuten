open data.json
| get items
| where status == active
| sort-by created_at
| select name email created_at
| first 10
open data.json | get items | where status == active | sort-by created_at | select name | first 10
# The first line of the last element does not fit, so the pipes break.
ls
| where name =~ "a very long pattern that takes up most of the line, and then some more"
| each {|entry|
      print $entry
  }
