export-env {
    $env.A = 1
}
export-env {
    $env.B = 2
}
export-env {}
module m {
    export-env {
        $env.D = 1
    }
}
