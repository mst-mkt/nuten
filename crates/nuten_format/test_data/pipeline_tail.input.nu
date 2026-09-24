ls | to json [
    a
]
ls | each {|x|
    print $x
}
ls | where type == dir | each {|x|
    print $x
}
ls |
each {|x|
    print $x
}
# The first line of the last element does not fit, so the pipes break.
ls | where name =~ "a very long pattern that takes up most of the line, and then some more" | each {|entry|
    print $entry
}
ls | str join "a
b"
# Only the last element counts by its first line.
ls | each {|x|
    print $x
} | length
# A pipe that ends the pipeline leaves every element to fit whole.
ls | to json [
    a
] e>|
ls | to json [
    a
] o+e>|
$alpha | $in + [$beta $gamma $delta $epsilon $zeta $eta $theta $iota $kappa $lambda $mu $nu $xi $omicron]
ls | where name in [alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi]
