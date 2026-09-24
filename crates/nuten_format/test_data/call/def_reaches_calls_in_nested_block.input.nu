module mm { def m4 [x] { $x }; export def m5 [] { m4 {a:  1} } }
def recursive [x] { recursive {a:  1} }
