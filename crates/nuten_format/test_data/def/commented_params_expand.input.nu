def g [
  # leading
  x # about x
  --flag (-f)
  --other  =  1
] {
  ls
}
def t [x: # c
 int] { ls }
def tf [x: record<a: int # c
>] { ls }
def tr [x: # c
 record<a: int>] { ls }
def ty [x # c
: int] { ls }
