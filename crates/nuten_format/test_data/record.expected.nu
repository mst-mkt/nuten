echo {a:  1,b: 2} {
  c: 3
      d: 4 }
{ a: 1 b: 2 } | to json
let r = { a: 1 b: 2 }
let s = { a: 1 b: [1 2] c: { d: 3 } }
let t = {
    a: 1 # one

    b: 2
}
let u = {
    a # c
    : 1
}
let v = {
    a: # c
    1
}
let w = { ...$env a: 1 }
[{ a: 1 } { b: 2 }]
{ a: 1 | b }
let long = {
    aaaaaaaaaaaaaaaa: 1111111111111111
    bbbbbbbbbbbbbbbbbbbb: 222222222222222
    ccccccccccccc: 3333333333333
}
echo {a: 1}
let x = { a: b:c }
let y = { "a b": 1 'c': 2 }
let r = {a:# [
[1]]}
{
    a: 1 # c
}
{
    a: 1

    b: 2
}
{a,b: 1}
{...$env,a: 1}
{ a: 1 }
{ a: : }
{ ...$env a: 1 }
{ z: 1 a b : c }
{ a: 1 }
{ a: 1 }
{,: 1}
{ ...$env a: 1 }
