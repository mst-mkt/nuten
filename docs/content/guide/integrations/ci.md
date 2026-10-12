---
title: CI
---

# CI

## GitHub Actions

> [!NOTE]
> The action is not yet published.

Install `nuten` with [setup-nuten](https://github.com/mst-mkt/setup-nuten) and report the result as annotations with `--reporter github`.

```yaml [.github/workflows/format.yaml]
on:
  pull_request:
  push:
    branches: [main]

jobs:
  format:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - uses: mst-mkt/setup-nuten@{sha} # {version}
      - run: nuten format --reporter github
```

`--reporter github` does not change any file. The lines that need formatting and the errors become annotations, which show up in the diff of a pull request. The job fails if any file needs formatting or has an error.

## GitLab CI

> [!NOTE]
> The install script is not yet published.

Install `nuten` with the install script and turn the result into a Code Quality report with `--reporter gitlab`.

```yaml [.gitlab-ci.yml]
format:
  script:
    - curl -fsSL https://nuten.keito.dev/install.sh | sh
    - export PATH="$HOME/.local/bin:$PATH"
    - nuten format --reporter gitlab > gl-code-quality-report.json
  artifacts:
    when: always
    reports:
      codequality: gl-code-quality-report.json
```

`--reporter gitlab` does not change any file and writes the report to stdout. The report lists the lines that need formatting and the errors, and it shows up in merge requests. The job fails if any file needs formatting or has an error, so `when: always` keeps the report in that case too.
