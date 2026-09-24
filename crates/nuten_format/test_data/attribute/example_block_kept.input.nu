@example "a" {  ls|where x  }
def f [] {}
@example "b" {
  "
    x
  " | str dedent
}
def g [] {}
@example "c" { ls } --result {a:  1}
def h [] {}
@example "a" {
	ls  # c
}
def tab [] {}
