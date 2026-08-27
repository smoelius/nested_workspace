# Changelog

## 2.0.1

- Expand documentation with "Unintentional lockfile updates" section ([3452e52](https://github.com/smoelius/nested_workspace/commit/3452e526215492d68b82b93e88f3ac7093923e0d))
- Avoid build directory deadlocks for dependent workspaces ([11d8db0](https://github.com/smoelius/nested_workspace/commit/11d8db01967b881519f42f5061567b369d85d39e))

## 2.0.0

- BREAKING: Rework argument handling. Specifically, forward `--frozen` and `--locked` when running `cargo build` or `cargo check` on a nested workspace. Also, fix a bug that caused `--workspace` to be passed twice to `cargo test` when the name of a nested workspace's containing package could not be determined. ([55d9509](https://github.com/smoelius/nested_workspace/commit/55d95095b9912a5af92164e11c531cb73a3f525a))
- BREAKING: Always pass `--offline` in commands run on nested workspaces. Note that this change makes the "refusing to build/check" warnings no longer relevant. Thus, such warnings are no longer emitted. ([8901fbb](https://github.com/smoelius/nested_workspace/commit/8901fbbf9201ac5b9f1cc6c33f07a6760126f701))

## 1.0.0

- BREAKING: Bump the major version so that future feature additions do not appear to be breaking changes.
- FEATURE: Add `cargo-nested` `--list` option to list nested workspaces. ([7106d16](https://github.com/smoelius/nested_workspace/commit/7106d16e52fdaa056165fef7af53f75edff0acff))

## 0.9.0

- FEATURE: Improved `cargo-nested` argument parsing. `cargo-nested` now supports `--help` and `--version`, for example. ([003798e](https://github.com/smoelius/nested_workspace/commit/003798e4d7ab5affe00a2af6bc5bbf519eb198a4))
- Dependency updates
  - `elaborate` upgraded to version 2.0

## 0.8.0

- FEATURE: Add `dependent` option to allow nested workspaces to depend on their containing packages ([#155](https://github.com/smoelius/nested_workspace/pull/155))

## 0.7.7

- Eliminate redundant work performed by `cargo nested check`, etc. Previously, such commands would cause additional executions of a nested workspace's build scripts. This is no longer the case. ([1e5cabb](https://github.com/smoelius/nested_workspace/commit/1e5cabb8a0ad8f1f203cc51bdcc3357c8078fecd))

## 0.7.6

- Expand "refusing to build/check" message ([91f1fa4](https://github.com/smoelius/nested_workspace/commit/91f1fa4ad0914cd3ea802706910d39b4cfd4a641))
- Canonicalize nested workspace roots. This fixes the handling of nested workspaces in sibling directories. ([21ca97a](https://github.com/smoelius/nested_workspace/commit/21ca97a0714276d4239853d33a101f308965b602))
- Dependency updates
  - `elaborate` upgraded to version 1

## 0.7.5

- Bump Rust version to 1.95
- Dependency updates
  - `elaborate` upgraded to version 0.2
  - `sysinfo` upgraded to version 0.39

## 0.7.4

- Verify the each nested workspace root contains a workspace at the named location ([#98](https://github.com/smoelius/nested_workspace/pull/98))

## 0.7.3

- Use `sysinfo` to get parent process arguments ([#86](https://github.com/smoelius/nested_workspace/pull/86))

## 0.7.2

- Parse shell commands with `shlex` ([#82](https://github.com/smoelius/nested_workspace/pull/82))

## 0.7.1

- Walk parent commands to find the Cargo command so that [runners](https://doc.rust-lang.org/cargo/reference/config.html#targettriplerunner) are supported ([#78](https://github.com/smoelius/nested_workspace/pull/78))

## 0.7.0

- FEATURE: Allow nested workspace roots to be named with `glob` patterns ([#70](https://github.com/smoelius/nested_workspace/pull/70))

## 0.6.0

- Add path to "Found no nested workspaces" message ([801ca96](https://github.com/smoelius/nested_workspace/commit/801ca9607a517da390d3a81e34fbbf624b21fe0c))
- FEATURE: Emit error message when a cycle is detected among nested workspaces ([#61](https://github.com/smoelius/nested_workspace/pull/61))
- FEATURE: Use `elaborate` for better error reporting ([#67](https://github.com/smoelius/nested_workspace/pull/67))
- Use PowerShell and `Get-CimInstance` rather than `wmic` to get parent process on Windows ([#69](https://github.com/smoelius/nested_workspace/pull/69))

## 0.5.0

- Improve warning message ([#41](https://github.com/smoelius/nested_workspace/pull/41))
- BREAKING: Rename `cargo-nw` to `cargo-nested` ([#46](https://github.com/smoelius/nested_workspace/pull/46), [#47](https://github.com/smoelius/nested_workspace/pull/47), and [#50](https://github.com/smoelius/nested_workspace/pull/50))
- FEATURE: Simplify use of `ps` so that `nested_workspace` works on Alpine Linux ([#49](https://github.com/smoelius/nested_workspace/pull/49))

## 0.4.0

- Show subcommand in `cargo-nw` error message ([#28](https://github.com/smoelius/nested_workspace/pull/28))
- Expand documentation regarding use of Git dependencies ([#23](https://github.com/smoelius/nested_workspace/pull/23))
- FEATURE: Use mtimes to ensure build script is always rerun ([#33](https://github.com/smoelius/nested_workspace/pull/33))
- FEATURE: Support Windows ([#21](https://github.com/smoelius/nested_workspace/pull/21))

## 0.3.1

- Correct examples in documentation ([#19](https://github.com/smoelius/nested_workspace/pull/19))

## 0.3.0

- BREAKING: Check whether `--offline` was passed to parent command (see [Known problem](https://github.com/smoelius/nested_workspace/?tab=readme-ov-file#known-problem-potential-deadlocks) for additional information) ([#14](https://github.com/smoelius/nested_workspace/pull/14))
- BREAKING: Change how arguments are handled (see [Argument handling](https://github.com/smoelius/nested_workspace/?tab=readme-ov-file#argument-handling) for additional information) ([c7ff4ba](https://github.com/smoelius/nested_workspace/commit/c7ff4ba785462b315ca39c9d414bad3ac64b69c4))

## 0.2.0

- Pass `--workspace` to `cargo build` and `cargo check` ([ca14592](https://github.com/smoelius/nested_workspace/commit/ca1459251fe58c7285176f8dd7eb605ea5e3bb06))
- Clear `CARGO` and `RUSTC` in addition to `RUSTUP_TOOLCHAIN` ([ae1f4a1](https://github.com/smoelius/nested_workspace/commit/ae1f4a17d4392ee555bdeb6bcb658941f307cfa8))
- BREAKING: Change API to allow arguments to be passed to subcommands ([4e74a9b](https://github.com/smoelius/nested_workspace/commit/4e74a9b6bf13ee543fc85eff698efefa5c598c1e))

## 0.1.1

- Fix reference to README.md ([fee0e4c](https://github.com/smoelius/nested_workspace/commit/fee0e4c2e1301cf8ed78fec5adc4e20af78561f7))
- Eliminate reliance on `ansi_term` ([3db5bcc](https://github.com/smoelius/nested_workspace/commit/3db5bccc7a82506d7905772ce12add8359bdf32e))

## 0.1.0

- Initial release
