# Releasing

How a release is cut: what its version says, what it ships, and the steps from
the default branch to a published release. [CHANGELOG.md](CHANGELOG.md) is its
record.

## Versions

A tag, `vX.Y.Z`, versions the release under semantic versioning. While the major
version is 0, a minor release may break a user who takes it, and a patch release
never does.

[BREAKING-CHANGES.md](BREAKING-CHANGES.md) says how to move across a breaking
change.

## What a Release Ships

- The GitHub Release, with the release's section of the changelog as its notes.
- Every crate, published to crates.io by trusted publishing, the library first.

Turn on Immutable Releases in the repository's settings, so a published
release's files and tag never change.

## Steps

1. Write what the release changes:

   ```sh
   RELEASE_VERSION=x.y.z just release
   ```

   Every `release-*` recipe runs: `release-git-changelog` writes the release's
   section of `CHANGELOG.md`.

   `release-cargo-bump` sets every crate's version, and the lock's.
2. Read the new section; fix a commit's subject by rewording the commit, not the
   file. Run `just check`.
3. Commit it as `chore(release): vx.y.z`, which the changelog leaves out; sign
   the tag, `git tag -s vx.y.z`; push the branch and the tag.
4. The tag starts `release.yml`: it builds what every `package-*` recipe
   packages on each platform, attests it, publishes the GitHub Release with the
   notes and every file attached, then runs every `publish-*` recipe.
5. Read the Release's page and its notes.

   Each crate's page on crates.io shows its README and its metadata, and docs.rs
   built it, as its builds page says. `cargo install --locked <crate>` installs
   the new version from crates.io.

   The book is deployed from the default branch's last `check` run.
