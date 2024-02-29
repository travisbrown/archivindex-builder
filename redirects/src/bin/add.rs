fn main() -> Result<(), std::io::Error> {
    Ok(())
}
/*
    let opts: Opts = Opts::parse();
    opts.verbose.init_logging()?;

    let stdin = std::io::stdin();
    let mut lines_by_prefix: HashMap<String, Vec<String>> = HashMap::new();
    let file_prefixes = redirects::file_prefixes();

    for line in stdin.lock().lines() {
        let line = line?;
        if let Some(prefix) = line
            .chars()
            .next()
            .map(|c| c.to_string())
            .filter(|value| file_prefixes.contains(value))
        {
            let lines = lines_by_prefix.entry(prefix).or_default();
            lines.push(line);
        } else {
            panic!("Invalid input line: {}", line);
        }
    }

    for (prefix, mut lines) in lines_by_prefix {
        lines.sort();
        lines.reverse();
        let reader = BufReader::new(File::open(format!("data/redirects-{}.csv", prefix))?);
        let mut writer = BufWriter::new(tempfile::NamedTempFile::new()?);

        for line in reader.lines() {
            let line = line?;

            while let Some(next_new_line) = lines.pop() {
                match next_new_line.cmp(&line) {
                    Ordering::Greater => {
                        lines.push(next_new_line);
                        break;
                    }
                    Ordering::Less => {
                        writeln!(writer, "{}", next_new_line)?;
                    }
                    Ordering::Equal => {}
                }
            }

            writeln!(writer, "{}", line)?;
        }

        lines.reverse();

        for line in lines {
            writeln!(writer, "{}", line)?;
        }

        let tmp_file = writer.into_inner()?;

        std::fs::copy(tmp_file.path(), format!("data/redirects-{}.csv", prefix))?;
    }

    Ok(())
}

#[derive(Parser)]
#[clap(name = "add", about, version, author)]
struct Opts {
    #[clap(flatten)]
    verbose: Verbosity,
}
*/
