def f [x: record<a: int b: int> y: oneof<int string>] {
    $x
}
let r: record<a: int b: int> = { a: 1 b: 2 }
