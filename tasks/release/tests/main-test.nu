use std/assert
use std/testing *

use support.nu [commit, git-in, put, repository, with-git]

const TASK = path self ..

@before-each
def setup []: nothing -> record {
    let dir = repository
    let task = $dir | path join tasks/release
    mkdir ($task | path dirname)
    cp --recursive $TASK $task
    open ($TASK | path join groups.nuon) | where name == vscode | to nuon | save --force (
        $task | path join groups.nuon
    )
    put $dir integrations/vscode/package.json "{\n  \"version\": \"0.0.0\"\n}\n" | ignore
    commit $dir 'init: a'

    { dir: $dir }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

def release [dir: path, ...args: string]: nothing -> record {
    with-git { do { cd $dir; ^nu tasks/release/main.nu ...$args } | complete }
}

def tag-vscode [dir: path]: nothing -> record {
    with-git { do { cd $dir; ^nu --commands 'source tasks/release/main.nu; main tag vscode | to json --raw' } | complete }
}

def release-minor [dir: path]: nothing -> nothing {
    release $dir prepare vscode minor | ignore
    commit $dir 'chore(release): vscode/v0.1.0'
    tag-vscode $dir | ignore
}

@test
def "main releases first version from prepare to notes" [] {
    let context = $in

    let prepared = release $context.dir prepare vscode minor
    let checked = release $context.dir check
    commit $context.dir 'chore(release): vscode/v0.1.0'
    let tagged = tag-vscode $context.dir
    let notes = release $context.dir notes vscode/v0.1.0

    assert equal [$prepared.exit_code, $checked.exit_code, $tagged.exit_code, $notes.exit_code] [0, 0, 0, 0]
    assert equal ($tagged.stdout | from json) [vscode/v0.1.0]
    assert equal ($notes.stdout | str trim) '🎉 Initial release'
}

@test
def "main releases second version with changes since first" [] {
    let context = $in
    release-minor $context.dir
    put $context.dir integrations/vscode/src.ts x | ignore
    commit $context.dir 'fix: x'

    let prepared = release $context.dir prepare vscode patch
    let notes = release $context.dir notes vscode/v0.1.1

    assert equal $prepared.exit_code 0
    assert equal ($notes.stdout | str trim) "### Fixes\n\n- x"
}

@test
def "main tag creates tag of group" [] {
    let context = $in

    release-minor $context.dir

    assert equal (git-in $context.dir tag --list | lines) [vscode/v0.1.0]
}

@test
def "main check exits with 2 when places disagree" [] {
    let context = $in
    let groups = $context.dir | path join tasks/release/groups.nuon
    let disagreeing = open $groups
        | update 0.versions { append { file: other.json, field: [version] } }
    put $context.dir other.json '{ "version": "0.0.1" }' | ignore
    $disagreeing | to nuon | save --force $groups

    let actual = release $context.dir check

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr 'the places of vscode disagree on its version'
}

@test
def "main check exits with 2 and message of lib when file of place is missing" [] {
    let context = $in
    rm ($context.dir | path join integrations/vscode/package.json)

    let actual = release $context.dir check

    assert equal $actual.exit_code 2
    assert equal ($actual.stderr | str trim) 'integrations/vscode/package.json does not exist'
}

@test
def "main group exits with 2 for tag of no group" [] {
    let context = $in

    let actual = release $context.dir group nope

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr 'nope is not the tag of any group'
}
