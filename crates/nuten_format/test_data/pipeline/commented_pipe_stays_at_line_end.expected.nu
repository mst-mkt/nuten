ls | # after a pipe
get name |
# own line after a pipe
# another
first
ls | # the rest breaks too
get name
| first
ls
| get name | # only this pipe stays at the line end
first
ls | # c
each {|x|
    $x
}
