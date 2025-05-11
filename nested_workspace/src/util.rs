use ansi_term::Style;
use std::{
    env::current_dir,
    io::{IsTerminal, Write},
    path::Path,
};

pub struct Delimiter<'a>(&'a Path);

impl<'a> Delimiter<'a> {
    pub fn new(path: &'a Path) -> Self {
        let self_ = Self(path);
        self_.write_message(true);
        self_
    }

    fn write_message(&self, opening: bool) {
        let message = format!(
            "{} {}",
            if opening { "<<<" } else { ">>>" },
            self.0.display()
        );
        // smoelius: Writing directly to `stderr` prevents capture by `libtest`.
        writeln!(
            std::io::stderr(),
            "{}",
            if std::io::stderr().is_terminal() {
                Style::new().bold()
            } else {
                Style::new()
            }
            .paint(message)
        )
        .expect("failed to write to stderr");
    }
}

impl Drop for Delimiter<'_> {
    fn drop(&mut self) {
        self.write_message(false);
    }
}

#[expect(dead_code)]
trait StripCurrentDir {
    fn strip_current_dir(&self) -> &Self;
}

impl StripCurrentDir for Path {
    fn strip_current_dir(&self) -> &Self {
        let Ok(current_dir) = current_dir() else {
            return self;
        };
        self.strip_prefix(current_dir).unwrap_or(self)
    }
}
