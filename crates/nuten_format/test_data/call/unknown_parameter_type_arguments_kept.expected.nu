def f [x: nosuch, y?] {}
f {} [a,b]
def g [x: closure(int), y?] {}
g {} [a,b]
def h [x: oneof<closure(int), list>] {}
h [a,b]
