def takes-list [x: list, y: string, --flag(-f): list<string>, ...rest] { $x }
takes-list [1,  2] [a,  b] -f [c,  [d  e]] [e  f]
