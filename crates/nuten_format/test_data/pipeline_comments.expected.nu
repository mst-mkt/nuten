ls | # after a pipe
get name |
# own line after a pipe
# another
first
ls # before a line-start pipe
| length
ls
# own line before a line-start pipe
| length
ls |
# own line between two pipes
| length
ls # before a line-start pipe
| # after that pipe
length
ls | # a pipe then a pipe
| get name
ls | # the rest breaks too
get name
| first
ls
| get name | # only this pipe stays at the line end
first
ls | # trailing spaces
get name
