use aib_downloader::Downloader;
use cli_helpers::prelude::*;
use std::io::{BufRead, BufReader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();
    opts.verbose.init_logging()?;

    let client = Downloader::default();

    for line in BufReader::new(std::io::stdin()).lines() {
        let line = line?;
        let fields = line.split(',').collect::<Vec<_>>();
        let timestamp = fields[0].parse()?;
        let url = fields[1];
        let digest = fields[2].parse()?;

        match client
            .resolve_redirect_shallow(url, timestamp, digest)
            .await
        {
            Ok((info, _, true)) => {
                println!("{},{}", digest, info.url);
            }
            Ok((_info, content, false)) => {
                log::error!("Invalid: {} {}: {}", digest, url, content);
            }
            Err(error) => {
                log::error!("{},{},{}: {}", digest, url, timestamp, error);
            }
        }
    }

    Ok(())
}

#[derive(Parser)]
#[clap(name = "lookup", about, version, author)]
struct Opts {
    #[clap(flatten)]
    verbose: Verbosity,
}
