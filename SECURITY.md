# Security policy

## Scope

This policy covers Nuten and everything released from this repository, including the editor extensions and plugins.
It also covers the dprint plugin released from [`mst-mkt/dprint-plugin-nuten`](https://github.com/mst-mkt/dprint-plugin-nuten).

A formatting result that changes the meaning of code, or a panic on some input, is a bug rather than a vulnerability.
Report it in a [regular issue](https://github.com/mst-mkt/nuten/issues/new/choose).
If crafted input could turn such a bug against someone else, report it privately as below.

## Supported versions

Security fixes go into the [latest release](https://github.com/mst-mkt/nuten/releases/latest) only.

## Reporting a vulnerability

Report vulnerabilities through the [private vulnerability reporting form](https://github.com/mst-mkt/nuten/security/advisories/new), not in public issues or pull requests.

Include as much of the following as you can.

- The affected component and version
- What an attacker can do, and under what conditions
- Steps to reproduce, with a proof of concept if you have one
- Any workaround you know of

## Response

1. You get a first reply within 7 days
2. Together we confirm the impact and agree on a fix and a disclosure date
3. A fixed version is released, and a [security advisory](https://github.com/mst-mkt/nuten/security/advisories) is published with credit to you unless you decline it
