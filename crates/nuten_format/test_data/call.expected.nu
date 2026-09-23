echo hello "a  b" 'c d' $"x(1 + 2)y" r#'raw'# --flag=1 -f --long 2
^ls -la ~/.config
ls o> out.txt e>> err.log
if $x {
    1
}
else {  2  }
^echo [a b] [[a]; [1]]
echo [a,  b]
docker ps --format {{.Names}}
alias open = ^open
open -a {Google Chrome}
alias catch = save
catch [a,  b]
do { nosuch {a:  1} }
all {a:  1}
all {...$x}
any {|x| $x }
ls; | @foo {a:  1}
ls | const {a:  1}
% ls [a,  b]
%echo { a: 1 }
echo { a: 1 } {...$x,  print hi}
let {ls  x} = 1
ls |
@foo {a:  1}
