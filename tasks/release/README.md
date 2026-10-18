# nuten-release

The release tasks of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell, which the workflows in `.github/workflows` run.

```nushell
nu tasks/release/main.nu --help
```

## Groups

What Nuten publishes is released in groups, and each group has one version.
`groups.nuon` defines the tag of each group, the paths its changes come from, and the places that hold its version.

| group    | tag             | publishes                                              |
| -------- | --------------- | ------------------------------------------------------ |
| `nuten`  | `v0.1.0`        | the CLI, `@nuten/format`, and `@nuten/prettier-plugin` |
| `vscode` | `vscode/v0.1.0` | the VS Code extension                                  |
| `zed`    | `zed/v0.1.0`    | the Zed extension                                      |

## Flow

1. Run `release-prepare` with a group and the part of its version to bump.
2. Edit the draft changelog in the pull request it opens from `release/<group>`.
3. Merge the pull request.
4. `release-tag` tags the merged commit for that group.
5. `release` publishes the group, then its GitHub Release.

### Recovery

- When a job of `release` fails, re-run the failed jobs of `release-tag`, which skip what is already published.
- Running `release-prepare` again rebuilds the pull request from main, which drops the edits made to it.

## Subcommands

### prepare

Bumps the version of a group, updates its lockfiles, and adds a section to its changelog.
The section is a draft that lists the `feat`, `add`, `fix`, and `perf` commits on the paths of the group since its newest tag, with breaking changes first.
It refuses a group whose version has no tag yet, except `0.0.0`, which every group holds before its first release and whose section says only 🎉 Initial release.

```nushell
nu tasks/release/main.nu prepare nuten minor
```

```nushell
{ group: nuten, version: 0.1.0, tag: v0.1.0 }
```

### check

Fails when the places of a group disagree on its version.

```nushell
nu tasks/release/main.nu check
```

### tag

Tags HEAD for a group when its version is newer than its newest tag, and lists the tag.

```nushell
nu tasks/release/main.nu tag nuten
```

```nushell
[v0.1.0]
```

### group

Prints the group of a tag.

```nushell
nu tasks/release/main.nu group vscode/v0.1.0
```

```
vscode
```

### notes

Prints the section of the changelog for a tag, without its heading.

```nushell
nu tasks/release/main.nu notes v0.1.1
```

```markdown
### Fixes

- **parser:** read a bare word after a pipe
```

## Tests

`vp run test:nu` runs the [nutest](https://github.com/vyadh/nutest) suites in `tests`.
