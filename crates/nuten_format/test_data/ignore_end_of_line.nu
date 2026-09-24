$env.config = { # nuten-ignore
  a:   1
}

let flags = [
  "--output" "json"   # nuten-ignore
  "--lines"  $n
]

if $a {
  1
}  else   { # nuten-ignore
  2
}

def main [] {
  if $a {
    1
  } else { # nuten-ignore
    let text = "a
b"
  }
}

let list = [
  1  2
]  # nuten-ignore

let names = (ls
  | where  size > 1  | get  name) # nuten-ignore

let record = {
  a: 1,  b: 2 # nuten-ignore
  c: 3,  d: 4
}

match $x {
  1 => 'one',  2 => 'two' # nuten-ignore
  3 => 'three'
}

def   g [] {
  a;  b # nuten-ignore
  c;  d
}

ls
| where  size > 1  | get  name # nuten-ignore
| first  2

(
  $a  +
  $b  +  $c # nuten-ignore
)

(
  ls
  | where  $it.size  >  1 # nuten-ignore
)

(foo
  --a  1  --b  2 # nuten-ignore
  --c  3
)

let table = [
  [a  b]; [1  2] # nuten-ignore
  [3  4]
]

(
  ls
).name.size   # nuten-ignore

ls | each {
  |x|  $x.name   # nuten-ignore
}

let bytes = 0x[
  01  02 # nuten-ignore
  03  04
]

$"(
1
)(2)(3)" # nuten-ignore

def h [
] { # nuten-ignore
  1
}

ls
| $x   =   1 # nuten-ignore

# nuten-ignore
let both = [1  2] # nuten-ignore

let nested = [
  [1  2] [ # nuten-ignore
    3  4 # nuten-ignore
  ]
]

def k [] {
  a;  ls  | # nuten-ignore


  | length
}

(ls | where
  $it.size  >  1 # nuten-ignore
)

alias x = foo |
  bar   baz # nuten-ignore

@example "x" {
  1
}   --result  1 # nuten-ignore
def y [] {}

def io []: [
  int   ->   int # nuten-ignore
  string -> string
] {}

let fields = {
  a:   1,   b:   2, # nuten-ignore
  c: 3
}

let crossed = [
  # nuten-ignore
  1  2 # nuten-ignore
  3  4
]

@example (
  "x"
) {  1  }  --result   1 # nuten-ignore
def z [] {}

(alias w =
  foo   --a    1 # nuten-ignore
)

$"(
1
)(  2  )  (  # nuten-ignore
3)"

(
    ls
    | let x   =   a # nuten-ignore
    b
)

let x = # c
| length   a # nuten-ignore

let at_line_start = [
[1  2] [3  4] # nuten-ignore
]

let other_parent = [
  [1
  2  ]  [3   4] # nuten-ignore
]

let after_bracket = [
  1
  2   3] # nuten-ignore

match 1 {
  1
  =>   2 # nuten-ignore
  _ => 3
}

let leading_colon = {
  a
  :   1 # nuten-ignore
}

def leading_eq [
  x: int
  = 3 # nuten-ignore
] {}
