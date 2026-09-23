@search-terms a | b || c o> d = e
def f [] {}
@search-terms aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
def g [] {}
@search-terms a
| b
def h [] {}
@example "a" {  ls|where x  }
def f [] {}
@example "b" {
  "
    x
  " | str dedent
}
def g [] {}
@example "c" { ls } --result { a: 1 }
def h [] {}
module m {
    @example "d" {
    ls
  }
    def i [] {}
}
@example "a" {
	ls  # c
}
def tab [] {}
use std/testing *
@test {a:  1}
def t [] {}
