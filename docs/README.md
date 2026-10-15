# nuten.keito.dev

The documentation site of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It is built with [Ox Content](https://github.com/ubugeeei-prod/ox-content) on Vite+.

## Development

The pages are Markdown files in `content`, with their Japanese versions at the same paths in `content/ja`.

```nushell
pnpm install
vp run @nuten/docs#dev
vp run @nuten/docs#build
```

## Deployment

The site runs on Cloudflare Workers, and this command deploys it.

```nushell
vp run @nuten/docs#deploy
```

CI deploys the site on every push to `main`, and deploys a preview for each pull request that changes the docs.
