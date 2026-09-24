ls | each {|acc, x: int = 0| $acc }
ls | each {|x: record<a: int, b: string>, ...rest: list<int>| $x }
