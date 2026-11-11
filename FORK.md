# ade-zjstatus

ade-zjstatus is a fork of [zjstatus](https://github.com/dj95/zjstatus) by Daniel Jankowski. It keeps everything
zjstatus does and adds two things to its tab bar, for running several coding agents in Zellij tabs.

> [!WARNING]
> Under development: none of the features below are released yet.

## Planned features

- **Tab groups.** Tabs named `prefix: name` are grouped under `prefix`. Grouping is optional: a tab without a
  prefix, or the only tab with its prefix, shows as a plain tab. Clicking a group's label expands or collapses it
  without leaving the current tab; clicking a tab goes to it. A group's tabs are kept next to each other.
- **Agent status.** Each tab shows a coloured icon for what its programs are doing: working, waiting for you, done
  or failed. The status comes from Zellij's own attention tracking (Zellij 0.46 and newer), which programs feed
  through the [OSC 7501](https://mitchellh.com/writing/program-status-osc7501) program status protocol. A
  collapsed group adds up the statuses of its tabs, and a widget shows the totals for the whole session.

## Compatibility

- All of upstream's options and widgets keep working, so existing configs work unchanged. The
  [upstream wiki](https://github.com/dj95/zjstatus/wiki) documents them.
- Releases will need Zellij 0.46 or newer.
- The plugins keep upstream's file names (`zjstatus.wasm`, `zjframes.wasm`) and its `zjstatus::` pipe messages,
  so switching between upstream and this fork only means changing the download URL.

## Versions

A release is tagged with the upstream release it is based on plus a fork counter: `v0.25.0-ade.1`,
`v0.25.0-ade.2`, then `v0.26.0-ade.1` after merging upstream 0.26.0. Upstream's own tags are never pushed to this
repository.

`just release` (it needs [cargo-edit](https://github.com/killercup/cargo-edit) for `cargo set-version`) picks the
next version, sets it in `Cargo.toml`, commits, tags and pushes. The tag starts the release workflow, which builds
both plugins and creates a draft release to publish by hand.

## Keeping up with upstream

```sh
git remote add upstream https://github.com/dj95/zjstatus.git   # once
git fetch upstream --tags
git merge upstream/main
```

To keep these merges easy, change upstream's files as little as possible: put new features in new files where
that works, and document the fork here rather than in the README.

## Development

See [Development](./README.md#-development) in the README.
