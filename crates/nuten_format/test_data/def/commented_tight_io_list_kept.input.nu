def a []: [ # c
    nothing -> int] { 1 }
def b []: [int -> record<a: int # c
>] { 1 }
def c []: [
    # c
    string -> string
    nothing -> int] { 1 }
def d []: [
    # nuten-ignore
    string   ->   string
    nothing -> int] { 1 }
module m {
def e []: [ # c
    nothing -> int] { 1 }
}
def f []: [string  ->  string, int  ->  record<a:int # c
>] { 1 }
