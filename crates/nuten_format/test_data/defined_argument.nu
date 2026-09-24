def takes-list [x: list, y: string, --flag(-f): list<string>, ...rest] { $x }
takes-list [1,  2] [a,  b] -f [c,  [d  e]] [e  f]
def untyped [x] { $x }
untyped [1,  2]
later [1,  2]
def later [x: list] { $x }
def --wrapped wrapped [...x] {}
wrapped [1,  2]
extern external [x: list]
external [1,  2]
def number [n = 1, ...r] {}
number -1 [1,  2]
def text-first [x: oneof<list<any>, string>] {}
text-first [1,  2]
def str [x] {}
str join [1,  2]
module imported { export def replaced [x: string] {} }
def replaced [x: list] {}
use imported replaced
replaced [1,  2]
def prefix [...x] {}
prefix longer [1,  2]
module later { export def "prefix longer" [] {} }
use later *
