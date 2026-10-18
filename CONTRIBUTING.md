# Contributing

## Before you start

Nuten is not an official Nushell project.
Report problems with Nuten here, not to [Nushell](https://github.com/nushell/nushell) or [nufmt](https://github.com/nushell/nufmt).

You can report a bug in an issue, or fix it directly in a pull request.
Report a vulnerability privately instead, as [SECURITY.md](./SECURITY.md) describes.

Changes to the formatting style start with an issue, since style is a matter of preference and needs discussion.
Ideas for features and integrations are welcome too, and also start with an issue.

### Minimal reproduction

Include a minimal reproduction with a bug report or fix, so that anyone can check it regardless of their environment.
A reproduction covers the following.

- The smallest input that shows the bug
- Any settings that differ from the defaults
- The command you ran, or the editor or tool you ran Nuten from
- What Nuten produced, and what you expected
- The version of Nuten, and of Nushell if relevant

## Development environment

The [`nix/dev`](./nix/dev) devShell provides the toolchain, the corpus, and the git hooks.
Enter it with `nix develop ./nix/dev`, or automatically with [direnv](https://direnv.net) through `.envrc`.

Nix is not required.
Where it is not available, such as on Windows, you can set up the tools yourself.
[`nix/dev/shells.nix`](./nix/dev/shells.nix) lists the tools, and [`rust-toolchain.toml`](./rust-toolchain.toml) and [`nix/dev/flake.lock`](./nix/dev/flake.lock) pin their versions.
If you do, describe your setup in your issue or pull request.

Formatting, linting, and testing are [Vite+](https://viteplus.dev) tasks, defined in [`vite.config.ts`](./vite.config.ts) and run with `vp run`.

| Task                 | What it does                                                |
| -------------------- | ----------------------------------------------------------- |
| `vp run format`      | Formats every file with treefmt                             |
| `vp run lint:rust`   | Runs clippy and rustdoc, including the Wasm targets         |
| `vp run lint:ts`     | Lints and type-checks the TypeScript                        |
| `vp run test:rust`   | Runs the Rust tests                                         |
| `vp run test:oracle` | Checks with Nushell that formatting does not change the AST |
| `vp run test:nu`     | Runs the Nushell tests                                      |
| `vp run test:ts`     | Runs the TypeScript tests                                   |

CI runs all of them on every pull request.
The README of each crate describes its tests and how to run them.

## Documentation

The documentation is written in English and Japanese.
No other languages are planned for now, as they could not be maintained.
When a pull request changes the content, you can leave any language you do not know to the maintainer rather than translating it yourself.

## Using AI

You may use AI to contribute.
You remain responsible for the content of what you submit.

Feel free to contribute, though contributions of extremely low quality may not be accepted.
