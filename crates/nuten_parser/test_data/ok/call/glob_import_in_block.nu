def imported-inside [x] { $x }
do { use m.nu *; imported-inside {a:  1} }
do { use m.nu *; each {a:  1} }
print {a:  1}
