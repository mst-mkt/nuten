for x: int in [1] { print $x }
for x: record<a: int> in [{a: 1}] { print $x }
for x: [int] in [1] { ls }
for x: int in in [1] { ls }
for x: oneof<int, string> in [1] { ls }
