for x: int in [1] {
    print $x
}
for x: record<a: int> in [{ a: 1 }] {
    print $x
}
for x: list<record<a: int, b: string>> in [] {
    print $x
}
