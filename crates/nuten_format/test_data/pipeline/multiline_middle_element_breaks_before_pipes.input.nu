$rows | each {|row|
  $row.name
} | str join
$rows | each {|row| $row.name } | str join
ls
| where name in [
  a
  b
] | length
# Only the last element counts by its first line.
ls | each {|x|
    print $x
} | length
ls | each {|x|
  $x
} | length
do { aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa; bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb } | complete
