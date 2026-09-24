module mm { def m4 [x] { $x }; export def m5 [] { m4 {a:  1} } }
def enclosing [] { nested {a:  1}; def nested [x] { $x } }
def recursive [x] { recursive {a:  1} }
