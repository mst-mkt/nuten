(echo use m *; do {|x| $x } 1)
def foo [x] {
    $x
}
(
    ls hide foo
)
foo {a:  1}
(
    open { x } alias catch = save
    catch [a, b]
)
