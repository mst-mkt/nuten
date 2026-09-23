match $x { {} => 1, {a:1,$b} => 2, {  a :  [ $c ,.. ] , d: {e: ( 1 )..3} } => 3, {a: 1} | {b: 2} => 4 }
match $x {
  {a # c
    : 1
    b:
    2,
  } => 1
  {aaaaaaaaaaaaaaaaaaaaaaaa: 1, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: 2, cccccccccccccccccccccccccccc: 3} => 2
}
