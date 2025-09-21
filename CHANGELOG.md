# Changelog

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
