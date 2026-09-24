(def paren [x] { $x }; paren {a:  1})
do {|y| late {a:  1}; def late [x] { $x } }
match 1 { 1 => { def in-arm [x] { $x }; in-arm {a:  1} }, _ => {} }
def enclosing [] { nested {a:  1}; def nested [x] { $x } }
