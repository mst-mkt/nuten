match $x { $y if ( $y|length )>1 => a, $y if $y in [1,2] and not $z => b, $y if $y | z => c }
