use clap::Parser;
use home;
use backer::server::Server;

#[derive(Parser)]
struct Opts {

    /// files backup server port
    #[clap(short = 'p', long, default_value = "9618")]
    port: String,

    /// files backup dir
    #[clap(short = 'b', long, default_value = "")]
    backup_dir: String,
}

fn main() {
    let mut opts = Opts::parse();
    if opts.backup_dir.is_empty() {
        match home::home_dir() {
            Some(path) => {
                let path = path.join("backer_dir");
                let path_str = path.to_str();
                match path_str {
                    Some(home_path) => {
                        println!("Use user home dir for backup dir");
                        opts.backup_dir = home_path.to_string()
                    },
                    None => {
                        panic!("Not parse your home dir!");
                    },
                }
            },
            None => panic!("Impossible to get your home dir!"),
        }
    }
    let bs = Server::new(opts.port.clone(), opts.backup_dir.clone());
    let _ = bs.start();

}