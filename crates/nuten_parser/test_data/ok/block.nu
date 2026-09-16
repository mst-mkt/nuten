ls | each {|x| $x.name }
ls | each { |x, y|
    print $x
}
{ ls } | length
echo {|| 1} {|x|} {||} {|x,
y| $x}
echo {
    # comment
    ls; pwd
    ls
    | length
}
echo { } {} { # c
}
echo {|x 1} {ls |} {a && b} {|x|; ls}
^echo {ls} o> {pwd}
echo {
|x| x}
echo {{a: 1}} {[1 2]} {{ls}} {ls}.a
echo {|x||} {|a ||} {|x && y| 1} {|; x| 1} {|x o>| y| 1} {|2>| x} {|a || b| 1} {|x 2>&1 y| 1} {|x out>| y| 1}
echo {#"
"}
echo {

|x| x}
echo { # c
|x| x}
