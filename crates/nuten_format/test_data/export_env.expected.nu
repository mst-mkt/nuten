export-env {
    $env.A = 1
}
export-env {
    $env.B = 2
}
export-env {}
export-env { $env.C = 1 } (ls   -a)
export-env
module m {
    export-env {
        $env.D = 1
    }
}
export-env {
    $env.A = 1
} o> x
export-env --help {  $env.A = 1 }
