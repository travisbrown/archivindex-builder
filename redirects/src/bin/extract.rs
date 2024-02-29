fn main() -> Result<(), std::io::Error> {
    Ok(())
}
/*
    let opts: Opts = Opts::parse();
    opts.verbose.init_logging()?;

    let mut by_digest = HashMap::new();

    let mut entries = std::fs::read_dir("data")?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let reader = BufReader::new(File::open(entry.path())?);

        for line in reader.lines() {
            let line = line?;
            let mut parts = line.split(',');
            if let Some((digest, url)) = parts.next().zip(parts.next()) {
                if let Some((screen_name, status_id)) = parse_tweet_url(url) {
                    by_digest.insert(digest.to_string(), (screen_name, status_id));
                }
            } else {
                log::error!("Invalid line: {}", line);
            }
        }
    }

    let reader = BufReader::new(File::open(opts.path)?);

    for line in reader.lines() {
        let line = line?;
        let mut parts = line.split(',');
        if let Some(((url, _), digest)) = parts.next().zip(parts.next()).zip(parts.next()) {
            if let Some((screen_name, status_id)) = parse_tweet_url(url) {
                if let Some((retweeted_screen_name, retweeted_status_id)) = by_digest.get(digest) {
                    println!(
                        "{},{},{},{}",
                        screen_name, retweeted_screen_name, status_id, retweeted_status_id
                    );
                }
            }
        } else {
            log::error!("Invalid retweet line: {}", line);
        }
    }

    Ok(())
}

pub fn parse_tweet_url(url: &str) -> Option<(String, u64)> {
    lazy_static::lazy_static! {
        static ref TWEET_URL_RE: Regex = Regex::new(TWEET_URL_PATTERN).unwrap();
    }

    TWEET_URL_RE.captures(url).and_then(|groups| {
        groups
            .get(1)
            .map(|m| m.as_str().to_string())
            .zip(groups.get(2).and_then(|m| m.as_str().parse::<u64>().ok()))
    })
}

#[derive(Parser)]
#[clap(name = "extract", about, version, author)]
struct Opts {
    #[clap(flatten)]
    verbose: Verbosity,
    //#[clap(subcommand)]
    //command: SubCommand,
    #[clap(short, long)]
    path: String,
}

#[derive(Parser)]
enum SubCommand {
    Digests,
}
*/
