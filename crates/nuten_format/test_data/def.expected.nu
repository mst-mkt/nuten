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
def ta [x: record<a: int, b: string>] {
    ls
}
def tb [x: record<a: int, b: list<record<c: int>>>] {
    ls
}
def tc [x: oneof<int, string>] {
    ls
}
def td [x: oneof<int, string>] {
    ls
}
def te [x: list<int>@"ls"] {
    ls
}
def tf [
    x: record<a: int # c
>
] {
    ls
}
def tg [x: record<a: int | b>] {
    ls
}
def th [x: listx<int>] {
    ls
}
def ti [x: record<name>] {
    ls
}
def tj [x: record<a: int>] {
    ls
}
def tk [x: table<a: int, b: string>] {
    ls
}
def tl [x: record<>] {
    ls
}
def tm [x: record<a, :, int>] {
    ls
}
def tn [x: record<a: ,, b: int>] {
    ls
}
def to [x: record<a: int>] {
    ls
}
def tp [x: record<a::, int>] {
    ls
}
def tq [x: record<a: int>] {
    ls
}
def tr [
    x: # c
    record<a: int>
] {
    ls
}
def ts [--flag(-f): record<a: int> = {a: 1}] {
    ls
}
def tt [
    aaaaaaaaaaaaaaaa: record<bbbbbbbbbbbbbbbb: int, cccccccccccccccc: string, dddddddddddddddd: int, eeeeeeeeeeeeeeee: int>
] {
    ls
}
def tu []: record<a: int> -> int {
    ls
}
def tv [x: list<int> = [1]] {
    ls
}
extern tw [x: record<a: int>]
def tx [x: record<a:>] {
    ls
}
def ty [
    x # c
    : int
] {
    ls
}
def tz [a,#c [
]
] {}
def tz2 [--f = --x (-y)] {
    ls
}
def tz3 [--f: --x (-y)] {
    ls
}
def tz4 [] {
    a: 1
}
def tz5 [] {
    ...$x, a: 1
}
def ua [] : {
    ls
}
extern ub [] :
extern uc [] : {}
def ud []: {
    ls
}
extern uz []:
def uf [] : {a} {
    ls
}
def ug []: {a} -> int {
    ls
}
def uh []: int string {
    ls
}
(
    def ui [] : {a} # c
    {
        ls
    }
)
extern uj [] : { ls }
def uk [] : {a} {}
def ul [] : {a} {a}->{b}
export def um [] : {a} {
    ls
}
export extern un [] : { ls }
