ls | each {|x| $x.name }
ls | each { |x, y|
    print $x
}
