Pick the next minor version
Current crate version is 0.1.1 in Cargo.toml:3, so the next minor is 0.2.0.

Bump the crate version
Edit Cargo.toml:3:
eg. version = "0.2.0"

Verify locally before tagging
Run:
cargo check
cargo build --release

Commit the release bump
Example:
git add Cargo.toml
git commit -m "release: v0.2.0 (magic sheet)"

Create a tag that matches the workflow trigger
Your workflow listens to tags matching v* in release.yml:5.
Use an annotated tag:
git tag -a v0.2.0 -m "EasyCue3 v0.2.0 - Magic Sheet"

Push branch and tag
git push origin master
git push origin v0.2.0

Let GitHub Actions build and publish
The release workflow in release.yml:1 will:
build Linux, Windows, macOS artifacts
create a GitHub Release with generated notes
attach packaged binaries