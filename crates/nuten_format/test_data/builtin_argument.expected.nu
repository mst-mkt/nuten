[1] | append [2 3]
[1] | append [[a b]; [1 2]]
print -n [a b] [c d]
print -- -n [a b]
default [1 2] | ignore
default [x] a b
glob --exclude [a b] "*.nu"
glob -e [[a] b] "*"
glob --exclude=[a  b] "*"
str join [a  b]
do {|x| $x } [1 2]
[1] | zip [1 2]
http post https://example.com [a b]
ansi gradient [a  b]
{ a: 1 } | merge deep [a b]
[1] | append [
    2
    3
]
glob -e [[a  b] c] "*"
[1] | %append [2 3]
