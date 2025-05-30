# Changelog

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
