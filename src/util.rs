use anstyle::Style;
use elaborate::std::{env::current_dir_wc, path::PathContext};
use std::{
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
        let style = if std::io::stderr().is_terminal() {
            Style::new().bold()
        } else {
            Style::new()
        };
        let message = format!(
            "{} {}",
            if opening { "<<<" } else { ">>>" },
            self.0.display()
        );
        // smoelius: Writing directly to `stderr` prevents capture by `libtest`.
        writeln!(std::io::stderr(), "{style}{message}{style:#}")
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
        let Ok(current_dir) = current_dir_wc() else {
            return self;
        };
        self.strip_prefix_wc(current_dir).unwrap_or(self)
    }
}
