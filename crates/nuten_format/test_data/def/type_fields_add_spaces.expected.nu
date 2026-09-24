def ta [x: record<a: int, b: string>] {
    ls
}
def tk [x: table<a: int, b: string>] {
    ls
}
extern tw [x: record<a: int>]
def d []: record<a: int, b: string> -> list<record<c: int>> {
    1
}
def e []: [list<any> -> nothing, list<any> -> table<old: path, new: path>] {
    1
}
