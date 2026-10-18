# Git reads the config of whoever runs the tests, whose signing or hooks would change what the tests see.
const GIT_ENV = {
    GIT_CONFIG_GLOBAL: /dev/null
    GIT_CONFIG_NOSYSTEM: '1'
    GIT_CONFIG_COUNT: '6'
    GIT_CONFIG_KEY_0: user.name
    GIT_CONFIG_VALUE_0: test
    GIT_CONFIG_KEY_1: user.email
    GIT_CONFIG_VALUE_1: test@example.com
    GIT_CONFIG_KEY_2: commit.gpgSign
    GIT_CONFIG_VALUE_2: 'false'
    GIT_CONFIG_KEY_3: tag.gpgSign
    GIT_CONFIG_VALUE_3: 'false'
    GIT_CONFIG_KEY_4: core.hooksPath
    GIT_CONFIG_VALUE_4: /dev/null
    GIT_CONFIG_KEY_5: init.defaultBranch
    GIT_CONFIG_VALUE_5: main
}

export def with-git [action: closure]: nothing -> any {
    with-env $GIT_ENV $action
}

export def --wrapped git-in [dir: path, ...args: string]: nothing -> string {
    with-git { ^git -C $dir ...$args }
}

export def put [dir: path, file: string, content: string]: nothing -> path {
    let path = $dir | path join $file
    mkdir ($path | path dirname)
    $content | save --force --raw $path

    $path
}

export def commit [dir: path, subject: string]: nothing -> nothing {
    git-in $dir add --all | ignore
    git-in $dir commit --quiet --allow-empty --message $subject | ignore
}

export def repository []: nothing -> path {
    let dir = mktemp --directory
    git-in $dir init --quiet | ignore

    $dir
}
