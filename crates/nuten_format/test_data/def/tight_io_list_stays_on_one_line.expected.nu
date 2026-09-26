def a []: [nothing -> int] {
    1
}
def b []: [string -> string, nothing -> int] {
    1
}
def c [
    --some-long-flag-name: string
    --another-long-flag-name: int
]: [string -> string, nothing -> int] {
    1
}
def d []: [any -> table, any -> record, any -> string, any -> list<string>, any -> record<a: int, b: int>] {
    1
}
def e []: [string -> string, nothing -> int] {
    1
}
export extern f []: [nothing -> int]
