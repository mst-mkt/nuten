docker ps --format {{.Names}}
printf {%s} [a,  b]
echo {a:  1} {...$x, print hi} [a,  b]
ls | @foo {a:  1}
ls; | @foo {a:  1}
% ls [a,  b]
%echo {a:  1}
%nosuch {a:  1}
%where {a:  1} [a,  b]
@example {a:  1} {  1 + 1  }
^ls {a:  1}
ls |
@foo {a:  1}
