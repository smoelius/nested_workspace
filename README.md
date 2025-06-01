# nested_workspace

Run Cargo commands on workspaces in workspaces

Nested Workspace supports the following Cargo subcommands directly:

- `cargo build`
- `cargo check`
- `cargo test`

Additional Cargo subcommands are supported via the `nw` subcommand, installed with the following command:[^1]

```sh
cargo install nested_workspace
```

[^1]: `cargo install cargo-nw` will install a [different subcommand], unrelated to Nested Workspace.

For example, the follow command runs `cargo clean` on the current package or workspace and each nested workspace:

```sh
cargo nw clean
```

**Note:** `cargo nw build` and `cargo nw test` should also work. However, they may result in extra calls to `cargo build` and `cargo test` (respectively) if direct support for these commands is configured (as describe next).

## Usage

Nested Workspace requires that each nested workspace appear under a _containing package_ as follows ([example]):

```
containing package
├─ nested workspace A
└─ nested workspace B
```

Furthermore, the following steps are required:

1. In the containing package's Cargo.toml file, create a `nest_workspace` metadata table. The table should contain a `roots` array with the name of each nested workspace. Example:

   ```toml
   [package.metadata.nested_workspace]
   roots = [
      "nested_workspace_a",
      "nested_workspace_b",
      ...
   ]
   ```

2. To enable direct support for `cargo build` and `cargo check`, add `nested_workspace` as `build-dependency` to the containing package's Cargo.toml:

   ```toml
   [build-dependencies]
   nested_workspace = "0.1"
   ```

   And create a build script (`build.rs`) with the following contents:

   ```rs
   fn main() {
       nested_workspace::build().unwrap();
   }
   ```

3. To enable direct support for `cargo test`, add `nested_workspace` as `dev-dependency` to the containing package's Cargo.toml:

   ```toml
   [dev-dependencies]
   nested_workspace = "0.1"
   ```

   And create a test like the following:

   ```rs
   #[test]
   fn nested_workspace() {
       nested_workspace::test().unwrap();
   }
   ```

## Argument handling

### `cargo build` and `cargo check`

All arguments are filtered out; no arguments are forwarded. However, the commands are called with `-vv`, `--offline`, and `--workspace`:

- `-vv` aids in debugging.

- `--offline` avoids potential deadlocks (see [Known problem] below).

- `--workspace` ensures all packages in a nested workspace are built/checked, even if a nested workspace contains a root package.

### `cargo test`

The following modifications are made:

- `-p <containing-package>` and `--package <containing-package>` are filtered out.

- All arguments besides those covered by the previous bullet are forwarded.

- `--workspace` is added to the arguments so that all packages in a nested workspace are tested, even if a nested workspace contains a root package.

### `cargo nw <subcommand>`

All arguments are forwarded; no arguments are filtered out or added.

A primary reason for this policy is that the arguments accepted by an arbitrary subcommand cannot be predicted. For example, a subcommand might not accept `--workspace`, or it might consider `-p` to mean something other than "package".

## Known problem: potential deadlocks

Nested Workspace has safeguards to avoid potential deadlocks.

A build script holds a lock on the build directory while running. Furthermore, `cargo check` tries to obtain a lock on the package cache unless `--offline` is passed. Thus, the following scenario could occur:

- Thread A runs `cargo check`, which locks the package cache, locks the build directory, and then releases the lock on the package cache.
- Thread B runs `cargo check`, which locks the package cache and tries to lock the build directory, but blocks because thread A holds the lock.
- Thread A runs the build script, which runs `cargo check` and tries to lock the package cache, but blocks because thread B holds the lock.

To avoid this scenario, Nested Workspace checks whether `--offline` was passed to the parent command (i.e., the Cargo command that caused the build script to be run). If not, Nested Workspace exits with a warning like the following:

```
Refusing to check as `--offline` was not passed to parent command
```

Thus, in the scenario above, thread A would not hold a lock on the package cache, thereby avoiding the deadlock.

## Git dependencies

Using `cargo check --offline` with Git dependencies can result in errors like the following:

```
error: failed to get `clippy_utils` as a dependency of ...
...
Caused by:
  can't checkout from 'https://github.com/rust-lang/rust-clippy': you are in the offline mode (--offline)
```

To avoid such errors, we recommend running `cargo nw fetch` beforehand, e.g.:

```sh
cargo nw fetch && cargo check --offline
```

## Why would one need multiple workspaces?

- **Multiple toolchains:** Cargo builds all targets in workspace [with the same toolchain]. If a project needs multiple toolchains, then multiple workspaces are needed. ([Dylint] is an example of such a project.)

- **Conflicting features:** Cargo performs [feature unification] across the packages in a workspace. Features are meant to be additive, but some packages have conflicting features ([`gix-transport`] is an example). Multiple workspaces can be used to build targets with features that conflict.

## Why aren't more subcommands supported directly?

Nested Workspace needs a _trigger_ to run a subcommand:

- For `cargo build` and `cargo check`, the trigger is a build script containing `nested_workspace::build()`.
- For `cargo test`, the trigger is a test containing `nested_workspace::test()`.

For other subcommands, there is no obvious trigger. Hence, other subcommands must be run with `cargo nw <subcommand>`.

[Dylint]: https://github.com/trailofbits/dylint
[Known problem]: #known-problem-potential-deadlocks
[`gix-transport`]: https://github.com/GitoxideLabs/gitoxide/blob/8c353ea00c805604113a567d2f5157be94cc9f28/gix-transport/src/client/blocking_io/http/mod.rs#L25-L26
[different subcommand]: https://github.com/aspectron/cargo-nw
[example]: ./example
[feature unification]: https://doc.rust-lang.org/cargo/reference/features.html#feature-unification
[with the same toolchain]: https://github.com/rust-lang/rustup/issues/1399#issuecomment-383376082
