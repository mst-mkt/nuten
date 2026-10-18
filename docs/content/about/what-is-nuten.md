---
title: What is Nuten?
---

# What is Nuten?

## Overview

Nuten is a formatter for Nushell. It brings `.nu` scripts and `.nuon` data files into a consistent style.

The name is a blend of "Nu" and "neaten", and is pronounced `/ˈnuːtən/`, close to "Newton".

## Background

Nushell already has a formatter, [nufmt](https://github.com/nushell/nufmt). Earlier versions of nufmt sometimes broke the syntax or dropped comments when formatting. Many of those issues have been fixed, but Nuten aims to prevent this kind of problem by design.

There was also [topiary-nushell](https://github.com/blindFS/topiary-nushell), a Nushell formatter built on Topiary, but it has since been archived.

## Design principles

Nuten avoids breaking code through both its design and its tests. It parses code into a lossless CST, which keeps everything from the original, including whitespace and comments, and rewrites the code from it. Files with syntax errors are left untouched.

On top of that, tests run on hand-written inputs, real-world scripts, and random inputs, and check that formatting loses no tokens or comments and that formatting the output again changes nothing. They also have Nushell parse the code before and after formatting, to check that Nuten reads code the way Nushell does and that the meaning stays the same. See [Correctness](/about/correctness) for details.

Nuten also aims to be fast and lightweight, and to keep its binary small. See [Performance](/about/performance) for details.

Finally, it aims to fit right into everyday development, and offers [installation through Nix](/guide/installation#nix) and [integrations with editors and tools](/guide/integrations).

## Differences from nufmt

Nuten is not meant to replace nufmt. It is another option among Nushell formatters.

nufmt is the official formatter, developed by the Nushell team. It uses nu-parser, the same parser as Nushell itself, so it never reads code differently from Nushell. Even so, the code it outputs can still parse into a different AST from the original, which changes what the code means. See [Files left out](/about/performance#files-left-out) for how often this happens in real-world scripts.

Nuten has its own parser, separate from Nushell's, so a chance remains that it reads some code differently from Nushell, and it may lag behind new syntax. To make up for that, it relies on its design and tests to avoid breaking code. Compared with nufmt, it is faster, with lower memory usage and a smaller binary size. It also offers features an official project might not adopt right away, such as integrations with specific tools and extras around formatting.

If you value matching how Nushell parses code or using the official tool, nufmt may suit you better. If you value speed or integrations with editors and tools, Nuten may be the better fit. If nufmt breaks your code, give Nuten a try.
