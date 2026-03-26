#![feature(rustc_private)]

mod driver;

// r[impl driver.mode-detection]
fn main() {
    if std::env::var("__WHISKER_DRIVER").is_ok() {
        driver::run();
    } else {
        eprintln!("usage: whisker check [--manifest-path <path>]");
        eprintln!("       (CLI not yet implemented — driver mode only)");
        std::process::exit(1);
    }
}
