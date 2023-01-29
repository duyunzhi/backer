use anyhow::Result;
use clap::{ArgAction, Parser};
use signal_hook::{consts::TERM_SIGNALS, iterator::Signals};

use backer::backer::backer::Backer;
use backer::init::init::init;
use backer::version;

#[derive(Parser)]
struct Opts {
    /// Specify config file location
    #[clap(short = 'c', long, default_value = "/etc/backer/backer.yaml")]
    config_file: String,

    /// Display the version
    #[clap(short, long, action = ArgAction::SetTrue)]
    version: bool,
}

const VERSION_INFO: &'static version::VersionInfo = &version::VersionInfo {
    name: "backer",
    version: "0.1.0",
    compiler: env!("RUSTC_VERSION"),
    compile_time: env!("COMPILE_TIME"),
};

#[cfg(unix)]
fn wait_on_signals() {
    let mut signals = Signals::new(TERM_SIGNALS).unwrap();
    signals.forever().next();
    signals.handle().close();
}

fn main() -> Result<()> {
    let opts = Opts::parse();

    if opts.version {
        println!("{}", VERSION_INFO);
        return Ok(());
    }
    init();

    let backer = Backer::new()?;
    backer.start(&opts.config_file)?;
    wait_on_signals();
    backer.stop();
    Ok(())
}
