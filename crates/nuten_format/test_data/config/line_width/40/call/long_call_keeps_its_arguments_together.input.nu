let url = "https://example.com"
http post --content-type application/json $url "a body"
(print "a string long enough to pass the line width")
(print --no-newline "a string long enough to pass the width")
([1 2 3] | each {|x| $x + 1 } | where $it > 2 | math sum)
(par-each --threads 4 --keep-order {|x| $x + 1 })
(some-command first second third fourth fifth (ls))
(do --ignore-errors { ls } first second third fourth)
(if $url == "https://example.com" { "first" } else { "second" })
(FOO=1 some-command first second third fourth fifth)
(1kb some-command first second third fourth fifth)
(^echo first second third fourth fifth sixth seventh)
$"text (echo first second third fourth fifth sixth seventh)"
(echo first second third fourth fifth sixth; echo first)
(my-command first second third fourth fifth sixth)
