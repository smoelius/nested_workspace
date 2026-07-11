use elaborate::std::env::var_wc;

pub const CARGO_NESTED_ENV: &str = "NESTED_WORKSPACE_CARGO_NESTED";

pub fn enabled() -> bool {
    var_wc(CARGO_NESTED_ENV).is_ok_and(|value| value != "0")
}
