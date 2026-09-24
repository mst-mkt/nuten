match $x {
  {a # c
    : 1
    b:
    2,
  } => 1
  {aaaaaaaaaaaaaaaaaaaaaaaa: 1, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: 2, cccccccccccccccccccccccccccc: 3} => 2
}
