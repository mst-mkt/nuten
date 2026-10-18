use lib/changelog.nu
use lib/places.nu
use lib/prepare.nu
use lib/shell.nu
use lib/tag.nu

const GROUPS_FILE = path self groups.nuon

# Prepares, checks, and tags the releases of what Nuten publishes, group by group.
def main []: nothing -> nothing {
    print (help main)
}

# Bump the version of a group, update its lockfiles, and add a section for it to its changelog.
def "main prepare" [
    name: string@group-names # The group to release
    part: string@bump-parts # The part of the version to bump
]: nothing -> record<group: string, version: string, tag: string> {
    run-or-exit {
        let root = shell root
        let group = find-group $name
        let released = tag released $group (shell tags $root)
        let subjects = if $released == null {
            []
        } else {
            shell subjects $root (tag name $group $released) $group.paths
        }
        let texts = shell read-files $root (prepare files $group)
        let today = date now | format date '%Y-%m-%d'

        let plan = prepare $group $texts $released $subjects $part $today

        shell write-files $root $plan.writes
        shell update-locks $root $plan.locks
        # A lock that still disagrees has a manifest the group does not list.
        places agreed $group (shell read-files $root (places files $group)) | ignore

        { group: $group.name, version: $plan.version, tag: $plan.tag }
    }
}

# Check that the places of each group agree on its version.
def "main check" []: nothing -> nothing {
    run-or-exit {
        let root = shell root

        groups
        | each {|group| places agreed $group (shell read-files $root (places files $group)) }
        | ignore
    }
}

# Tag HEAD for a group when its version is newer than its newest tag, and list the tag.
def "main tag" [
    name: string@group-names # The group to tag
]: nothing -> list<string> {
    run-or-exit {
        let root = shell root
        let group = find-group $name
        let texts = shell read-files $root (places files $group)

        let due = tag due-tag $group $texts (shell tags $root)
        if $due != null {
            shell create-tag $root $due
        }

        [$due] | compact
    }
}

# The group that a tag belongs to.
def "main group" [
    tag: string # The tag, such as v0.1.0 or vscode/v0.1.0
]: nothing -> string {
    run-or-exit { parse-tag $tag | get group }
}

# The section of the changelog for a tag, without its heading.
def "main notes" [
    tag: string # The tag, such as v0.1.0 or vscode/v0.1.0
]: nothing -> string {
    run-or-exit {
        let root = shell root
        let parsed = parse-tag $tag
        let file = (find-group $parsed.group).changelog

        shell read-files $root [$file] | places existing-text $in $file | changelog notes $parsed.version
    }
}

def groups []: nothing -> table {
    open $GROUPS_FILE
}

def find-group [name: string]: nothing -> record {
    let found = groups | where name == $name | get 0?
    if $found == null {
        error make { msg: $"no group is named ($name)" }
    }

    $found
}

def parse-tag [tag: string]: nothing -> record<group: string, version: string> {
    let parsed = tag identify (groups) $tag
    if $parsed == null {
        error make { msg: $"($tag) is not the tag of any group" }
    }

    $parsed
}

def group-names []: nothing -> list<string> {
    groups | get name
}

def bump-parts []: nothing -> list<string> {
    [patch, minor, major]
}

# Each subcommand exits only here, since nu cannot exit from inside each.
# The innermost message is shown, since nu wraps an error from inside each in one of its own.
def run-or-exit [action: closure]: nothing -> any {
    try {
        do $action
    } catch {|error|
        print --stderr (innermost $error.details)
        exit 2
    }
}

def innermost [details: record]: nothing -> string {
    let inner = $details.inner? | default []

    if ($inner | is-empty) {
        $details.msg
    } else {
        innermost ($inner | first)
    }
}
