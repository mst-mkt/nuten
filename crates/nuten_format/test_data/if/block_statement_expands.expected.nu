ls | each {|f|
    if $f.size > 1kb {
        print big
    }
    $f
}
do {
    if $a {
        1
    } else {
        2
    };
}
ls | each {|x|
    if $x.a {
        print a
    }; $x
}
for it in $items {
    if $a {
        $s ++= [1]
    }
    if $b {
        $s ++= [2]
    }
}
