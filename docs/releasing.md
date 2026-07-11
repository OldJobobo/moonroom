# Releasing Moonroom 0.1

Moonroom 0.1 initially supports **Linux x86_64** hosts. The standalone builder embeds the current host executable and is not cross-compilation packaging.

## Install

For development or a local install:

```bash
cargo install --path crates/mr-cli --locked
moonroom --help
```

For a release artifact, download the `moonroom-VERSION-x86_64-unknown-linux-gnu` binary for your platform, make it executable, and place it on `PATH`.

## Release profile

Run the same release gate locally and in CI:

```bash
./scripts/release-check.sh
```

It validates formatting, Clippy, workspace tests, both source games, the packaged showcase, a standalone showcase smoke test, and the complete starter workflow.

Create versioned distributables and checksums on Linux x86_64:

```bash
./scripts/release-artifacts.sh 0.1.0
sha256sum -c dist/moonroom-0.1.0/SHA256SUMS
```

The output contains the Moonroom CLI, the House Under Glass `.moon` package, a standalone showcase executable, and `SHA256SUMS`. Tag the source revision used for the artifacts; that tag plus the checksum file identifies the release inputs.
