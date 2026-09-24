ls | each {|x|
  $x
}
ls | to json [
    a
]
ls | each {|x|
    print $x
}
ls | where type == dir | each {|x|
    print $x
}
