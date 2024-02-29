use aib_core::digest::Sha1Computer;
use cli_helpers::prelude::*;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("CLI argument reading error")]
    Args(#[from] cli_helpers::Error),
}

fn main() -> Result<(), Error> {
    let opts: Opts = Opts::parse();
    opts.verbose.init_logging()?;

    match opts.command {
        Command::ExportDigests => {
            let mut entries = std::fs::read_dir(opts.data)?.collect::<Result<Vec<_>, _>>()?;
            entries.sort_by_key(|entry| entry.path());

            for entry in entries {
                let reader = BufReader::new(File::open(entry.path())?);

                for line in reader.lines() {
                    let line = line?;
                    let mut parts = line.split(',');
                    if let Some(digest) = parts.next() {
                        println!("{}", digest);
                    } else {
                        log::error!("Invalid line: {}", line);
                    }
                }
            }
        }
        Command::Validate => {
            let mut entries = std::fs::read_dir(opts.data)?.collect::<Result<Vec<_>, _>>()?;
            entries.sort_by_key(|entry| entry.path());
            let mut saw_invalid = false;

            match entries.len().cmp(&32) {
                Ordering::Greater => {
                    log::error!("Too many files in data directory ({})", entries.len());
                    saw_invalid = true;
                }
                Ordering::Less => {
                    log::error!("Too few files in data directory ({})", entries.len());
                    saw_invalid = true;
                }
                _ => {}
            }

            for entry in entries {
                if !aib_redirects::is_valid_path(entry.path()) {
                    log::error!("Invalid file name: {:?}", entry.path());
                    saw_invalid = true;
                }

                if !print_validation_messages(entry.path())? {
                    saw_invalid = true;
                } else {
                    log::info!("Done: {:?}", entry.path());
                }
            }

            if saw_invalid {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

fn print_validation_messages<P: AsRef<Path> + Clone + Debug>(
    path: P,
) -> Result<bool, std::io::Error> {
    let (bad, is_sorted) = validate(path.clone())?;
    let bad_is_empty = bad.is_empty();

    if !is_sorted {
        log::error!("File is not sorted: {:?}", path);
    }

    if !bad_is_empty {
        log::error!("Invalid content in {:?} ({} lines)", path, bad.len());
        for line in bad {
            println!("{}", line);
        }
    }

    Ok(is_sorted && bad_is_empty)
}

fn validate<P: AsRef<Path> + Clone>(path: P) -> Result<(Vec<String>, bool), std::io::Error> {
    let mut computer = Sha1Computer::default();
    let mut bad = vec![];
    let mut is_sorted = true;
    let file = BufReader::new(std::fs::File::open(path)?);

    let mut lines = file.lines();
    if let Some(first) = lines.next() {
        let first = first?;

        if !validate_line(&mut computer, &first)? {
            bad.push(first.clone());
        }

        let mut last_seen = first;

        for line in lines {
            let line = line?;

            if line <= last_seen {
                is_sorted = false;
            }

            if !validate_line(&mut computer, &line)? {
                bad.push(line.clone());
            }

            last_seen = line;
        }
    }

    Ok((bad, is_sorted))
}

fn validate_line(computer: &mut Sha1Computer, input: &str) -> Result<bool, std::io::Error> {
    let mut parts = input.split(',');
    if let Some((digest, url)) = parts.next().zip(parts.next()) {
        let content = aib_core::redirect::make_redirect_html(url);
        let mut bytes = content.as_bytes();
        let computed_digest = computer.digest(&mut bytes)?;

        Ok(digest == computed_digest.to_string())
    } else {
        Ok(false)
    }
}

#[derive(Parser)]
#[clap(name = "redirects", about, version, author)]
struct Opts {
    #[clap(flatten)]
    verbose: Verbosity,
    #[clap(long)]
    data: PathBuf,
    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser)]
enum Command {
    ExportDigests,
    Validate,
}
