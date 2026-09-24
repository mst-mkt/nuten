export-env { $env.C = 1 } (ls   -a)
export-env
export-env {...$x}
export-env $x
export-env --help { $env.A = 1 }
