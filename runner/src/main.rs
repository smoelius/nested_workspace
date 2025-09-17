use std::{
    env::args,
    process::{Command, exit},
};

fn main() {
    let args = args().collect::<Vec<_>>();
    assert!(args.len() >= 2, "expect at least one argument");
    eprintln!("running: {:?}", &args[1..]);
    let mut command = Command::new(&args[1]);
    command.args(&args[2..]);
    let status = command.status().unwrap();
    if !status.success() {
        let code = status.code().unwrap();
        exit(code);
    }
}
