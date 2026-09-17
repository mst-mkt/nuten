def f [] {
    ls
}
export def --env "a b" [x: int y?: string@"c" ...rest: list<int> --flag(-g): int = 1] {
    ls
}
def g [
    # leading
    x # about x
    --flag(-f)
    --other = 1
] {
    ls
}
extern h []
export extern "git push" [remote?: string]
def i []: nothing -> string {
    "a"
}
def j []: int -> int {
    1
}
def k [
    a: int

    b: int
] {
    ls
}
def long [
    aaaaaaaaaaaaaaaaaaaa: string
    bbbbbbbbbbbbbbbbbbbb: string
    cccccccccccccccccccc: string
    dddddddddddddddddddd: string
] {
    ls
}
def m [x = {a: 1}] {
    ls
}
extern n [] {  ls  }
def o [x] {}
def p [] {
    ls
}
def : [] {
    ls
}
def q [] : : int -> string {
    ls
}
def r [--flag (-fg)] {
    ls
}
def s [x (-f)] {
    ls
}
def t [
    x: # c
    int
] {
    ls
}
extern u [--flag = 1]
def v [-- (-f)] {
    ls
}
extern w []: bar
extern x [] bar
