export def put [dir: path, file: string, content: oneof<string, binary>]: nothing -> path {
    let path = $dir | path join $file
    mkdir ($path | path dirname)
    $content | save --force $path
    $path
}

export def nuten-formatter [nuten: path]: nothing -> record {
    {
        name: nuten
        command: $nuten
        file: {|file| [$nuten, format, --stdin-filepath, $file, -] }
        directory: {|dir| [$nuten, format, --list-different, $dir] }
    }
}
