def a []: [int -> string, nothing -> int] {
    1
}
def b []: [nothing -> list<string>] {
    1
}
def c []: [
    nothing -> string,
    nothing -> table
] {
    1
}
def d []: record<a: int, b: string> -> list<record<c: int>> {
    1
}
def e []: [list<any> -> nothing, list<any> -> table<old: path, new: path>] {
    1
}
def f []: int->int {
    1
}
def g []: int -> int {
    1
}
def h []: [int -> string, nothing -> int] {
    1
}
def i []: int -> string, nothing -> int {
    1
}
export def --wrapped main [...arguments: string@"nu-complete jc"]: [
    any -> table,
    any -> record,
    any -> string
] {
    1
}
def j []: [
    # c
    int -> int, # d

    string -> string
] {
    1
}
def k []: [int # c
-> int] {
    1
}
def l []: [] {
    1
}
def m []: [
    int -> int
] {
    1
}
def n []: [int -> int, string -> string] {
    1
}
extern o []: [int -> int, string -> string]
def p []: int -> string] {
    1
}
def q []: [int -> int] {
    1
}
def s []: [int | string -> int] {
    1
}
(
    def u []: int -> int {
        1
    }
)
def v []: list<int>@c -> int {
    1
}
def w []: [
    int -> record<a: int # c
>
] {
    1
}
extern x []: int -> int
def y []: [
    int -> int,
    string -> string # e
] {
    1
}
def z []: [int, -> int] {
    1
}
def s []: , {
    1
}
(
    def t []: int # c
-> int {
        1
    }
)
(
    def u2 []: int -> int # c
string -> string {
        1
    }
)
