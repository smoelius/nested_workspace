# nested_workspace

Run Cargo commands on workspaces in workspaces

Nested Workspace supports the following Cargo subcommands directly:

- `cargo build`
- `cargo check`
- `cargo test`

Additional Cargo subcommands are supported via the `nested` subcommand, installed with the following command:

```sh
cargo install cargo-nested
```

For example, the follow command runs `cargo clean` on the current package or workspace and each nested workspace:

```sh
cargo nested clean
```

`cargo nested build` and `cargo nested test` also work. While `cargo nested` is running, direct support configured as described below is disabled so that it does not cause additional builds or tests of nested workspaces.

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
      { path = "nested_workspace_b", dependent = true },
      ...
   ]
   ```

   A root can be either a path string or a table containing `path` and `dependent`. If a nested
   workspace depends on the containing package, set `dependent = true` so that reentering the
   containing package's build script exits silently instead of failing the build with a
   workspace-cycle error. The default is `false`.

2. To enable direct support for `cargo build` and `cargo check`, add `nested_workspace` as `build-dependency` to the containing package's Cargo.toml:

   ```toml
   [build-dependencies]
   nested_workspace = "*"
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
   nested_workspace = "*"
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

- The following arguments are prepended to the arguments passed: `-vv`, `--offline`, and `--workspace`.
  - `-vv` aids in debugging.
  - `--offline` helps to avoid deadlocks (see [Potential deadlocks] below).
  - `--workspace` ensures all packages in a nested workspace are built/checked, even if a nested workspace contains a root package.

- The following arguments are forwarded: `--frozen` and `--locked`.

- All arguments besides those covered by the previous bullet are filtered out, i.e., no other arguments are forwarded.

### `cargo test`

- The following arguments are prepended to the arguments passed: `--offline` and `--workspace`. (The reason for prepending these arguments is to ensure they do not appear after `--` and are thus rejected by `libtest`.)
  - `--offline` helps to avoid deadlocks (see [Potential deadlocks] below).
  - `--workspace` ensures all packages in a nested workspace are built/checked, even if a nested workspace contains a root package.

- The following arguments are filtered out: `-p <containing-package>` and `--package <containing-package>`.

- All arguments besides those covered by the previous bullet are forwarded.

### `cargo nested <subcommand>`

All arguments are forwarded; no arguments are filtered out or added.

A primary reason for this policy is that the arguments accepted by an arbitrary subcommand cannot be predicted. For example, a subcommand might not accept `--workspace`, or it might consider `-p` to mean something other than "package".

## Known problems

### Potential deadlocks

Nested Workspace has safeguards to avoid potential deadlocks.

A build script holds a lock on the build directory while running. Furthermore, `cargo check` tries to obtain a lock on the package cache unless `--frozen` or `--offline` is passed. Thus, the following scenario could occur:

- Thread A runs `cargo check`, which locks the package cache, locks the build directory, and then releases the lock on the package cache.
- Thread B runs `cargo check`, which locks the package cache and tries to lock the build directory, but blocks because thread A holds the lock.
- Thread A runs the build script, which runs `cargo check` and tries to lock the package cache, but blocks because thread B holds the lock.

To avoid this scenario, Nested Workspace always passes `--offline` in commands run on nested workspaces.

Thus, in the scenario above, thread A would not hold a lock on the package cache, thereby avoiding the deadlock.

#### Git dependencies

Using `cargo check --offline` with Git dependencies can result in errors like the following:

```
error: failed to get `clippy_utils` as a dependency of ...
...
Caused by:
  can't checkout from 'https://github.com/rust-lang/rust-clippy': you are in the offline mode (--offline)
```

To avoid such errors, we recommend running `cargo nested fetch` beforehand, e.g.:

```sh
cargo nested fetch && cargo check --offline
```

### Unintentional lockfile updates

Running `cargo build` or `cargo check` on a containing package can cause unintentional updates to nested workspace lockfiles.

Nested Workspace tries to adopt a policy consistent with Cargo. That is, just as Cargo allows a user to opt out of updating a lockfile by passing `--frozen` or `--locked`, Nested Workspace forwards `--frozen` and `--locked` to `cargo build` and `cargo check` commands run on nested workspaces.

To adapt the direct `cargo build` and `cargo check` example from the [Usage] section, use [`Builder`]'s [`arg`] method to pass `--locked`:

```rs
fn main() {
    nested_workspace::build().arg("--locked").unwrap();
}
```

This causes Nested Workspace to run `cargo build` or `cargo check` on nested workspaces with `--locked`. If a nested workspace's lockfile needs to be updated, the command will fail rather than update the lockfile.

## Why would one need multiple workspaces?

- **Multiple toolchains:** Cargo builds all targets in workspace [with the same toolchain]. If a project needs multiple toolchains, then multiple workspaces are needed. ([Dylint] is an example of such a project.)

- **Conflicting features:** Cargo performs [feature unification] across the packages in a workspace. Features are meant to be additive, but some packages have conflicting features ([`gix-transport`] is an example). Multiple workspaces can be used to build targets with features that conflict.

## Why aren't more subcommands supported directly?

Nested Workspace needs a _trigger_ to run a subcommand:

- For `cargo build` and `cargo check`, the trigger is a build script containing `nested_workspace::build()`.
- For `cargo test`, the trigger is a test containing `nested_workspace::test()`.

For other subcommands, there is no obvious trigger. Hence, other subcommands must be run with `cargo nested <subcommand>`.

[Dylint]: https://github.com/trailofbits/dylint
[Potential deadlocks]: #potential-deadlocks
[Usage]: #usage
[`Builder`]: https://docs.rs/nested_workspace/latest/nested_workspace/struct.Builder.html
[`arg`]: https://docs.rs/nested_workspace/latest/nested_workspace/struct.Builder.html#method.arg
[`gix-transport`]: https://github.com/GitoxideLabs/gitoxide/blob/8c353ea00c805604113a567d2f5157be94cc9f28/gix-transport/src/client/blocking_io/http/mod.rs#L25-L26
[example]: ./example
[feature unification]: https://doc.rust-lang.org/cargo/reference/features.html#feature-unification
[with the same toolchain]: https://github.com/rust-lang/rustup/issues/1399#issuecomment-383376082
