def f [] { ls }
export def --env "a b" [x: int, y?: string@"c", ...rest: list<int>, --flag(-g): int = 1] { ls }
def g [
  # leading
  x # about x
  --flag (-f)
  --other = 1
] {
  ls
}
extern h []
export extern "git push" [remote?: string]
def i []: nothing -> string { "a" }
def j [] : int -> int { 1 }
def k []: [int -> string, nothing -> int] { "a" }
def l [x = {a: 1} y: record<a: int>] { ls }
def m [x | y ; z] { ls }
def n [x: int = 1 : int] { ls }
def o [--a(-b)(-c) --d(f) (-e)] { ls }
def "p[q]" [] { ls }
def q [] {} { ls }
extern r [] { ls }
def s []
def t [] --env { ls }
export def u [] { ls } # c
def --wrapped v [...rest] { ls }
def w [x: # c
] { ls }
def x [y = # c
] { ls }
def y [z: ;] { ls }
extern z [] {  ls  }
extern aa [] : bar
def ta [x: record<a:int,b:string>] { ls }
def tb [x: oneof<int string>] { ls }
def tc [x: record<
  a: int
  b: list<record<c: int>>
>] { ls }
def td [x: record<a: int # c
>] { ls }
def te [x: listx<int>] { ls }
def tf [x: list<int>@"ls"] { ls }
def tg [x: record<a,: int>] { ls }
def th [x: table<a: int>] { ls }
def ti [x: record<name>] { ls }
def tj [x: record<a:
int>] { ls }
def tk [x: record<a:,>] { ls }
def tl [x: record<a::>] { ls }
def tm [x: record<a>b] { ls }
def tn [x: record<a
: int>] { ls }
def to [x: record<>] { ls }
def tp []: record<a: int> -> int { ls }
def tq [x: "record<a: int>"] { ls }
def tr [x: record<a: "b>c">] { ls }
def ts [x: record<a:>] { ls }
