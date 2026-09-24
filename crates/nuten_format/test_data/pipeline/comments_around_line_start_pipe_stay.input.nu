ls   # before a line-start pipe
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
