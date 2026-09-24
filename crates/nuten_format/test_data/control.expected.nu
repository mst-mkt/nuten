for x in [1 2] {
    print $x
}
for $x in 1..3 {
    $x
}
for x in (ls) {
    $x; $x
}
while $a < 3 {
    $a += 1
}
loop {
    break
}
try {
    ls
}
try {
    ls
} catch {
    print $in
}
try {
    ls
} catch {|e|
    print $e
}
let x = try { 1 } catch { 2 }
let x = try {
    print a; 1
} catch {
    2
}
let long = try {
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
} catch {
    bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
}
ls | try { ls } catch { 2 } | length
do { try { 1 } catch { 2 } }
def f [] {
    for x in $xs {
        print $x
    }
}
for x in [1] {
    ls
} # c
try {
    ls
} # c
catch { ls }
for x [1] { ls }
for x in [1] { a: 1 }
try {} catch {}
for x in [1] {}
for x in [1] { # c
    ls
}
loop {
    try { ls } catch { pwd }
}
for x in $xs {
    for y in $ys {
        print $y
    }
}
let x = try {
    1
} catch {
    print e; 2
}
try {
    ls
} catch { # c
    pwd
}
do {
    try {
        1
    } catch {
        2
    };
}
for x: int in [1] {
    print $x
}
for x: record<a: int> in [{ a: 1 }] {
    print $x
}
for x: list<record<a: int, b: string>> in [] {
    print $x
}
for {a} in [1] {
    ls
}
for x in [1 2] {
    print $x
}
if [1 2] == $x {
    1
}
while [1 2] == $x {
    1
}
if true {
    ...$x, a: 1
}
for x in [1] {
    ...$x, a: 1
}
try {
    ...$x
} catch {
    ...$x, a: 1
}
while -h $a {
    1
}
while if true { true } else { false } {
    1
}
while { 1; 2 } {
    1
}
let y = if { 1; 2 } { 3 }
