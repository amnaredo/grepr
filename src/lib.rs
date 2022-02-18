use clap::{App, Arg};
use regex::{Regex, RegexBuilder};
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use text_colorizer::*;
use walkdir::WalkDir;

type MyResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub struct Config {
    pattern: Regex,
    files: Vec<String>,
    recursive: bool,
    count: bool,
    invert_match: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            pattern: Regex::new("").unwrap(),
            files: vec![],
            recursive: false,
            count: false,
            invert_match: false,
        }
    }
}

pub fn get_args() -> MyResult<Config> {
    let matches = App::new("grepr")
        .version("0.1.0")
        .author("Alejandro Martinez <amnaredo@gmail.com>")
        .about("Rust grep")
        .arg(
            Arg::with_name("pattern")
                .value_name("PATTERN")
                .help("Search pattern")
                .required(true),
        )
        .arg(
            Arg::with_name("files")
                .value_name("FILE")
                .help("Input file(s)")
                .required(true)
                .default_value("-")
                .min_values(1),
        )
        .arg(
            Arg::with_name("insensitive")
                .value_name("INSENSITIVE")
                .help("Case-insensitive")
                .short("i")
                .long("insensitive")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("recursive")
                .value_name("RECURSIVE")
                .help("Recursive search")
                .short("r")
                .long("recursive")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("count")
                .value_name("COUNT")
                .help("Count occurrences")
                .short("c")
                .long("count")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("invert")
                .value_name("INVERT")
                .help("Invert match")
                .short("v")
                .long("invert-match")
                .takes_value(false),
        )
        .get_matches();

    let mut config = Config::default();

    let pattern = matches.value_of("pattern").unwrap();
    let insensitive = matches.is_present("insensitive");
    let regex = RegexBuilder::new(pattern)
        .case_insensitive(insensitive)
        .build()
        .map_err(|_| format!("Invalid pattern \"{}\"", pattern))?;
    config.pattern = regex;

    config.files = matches.values_of_lossy("files").unwrap();

    config.recursive = matches.is_present("recursive");
    config.count = matches.is_present("count");
    config.invert_match = matches.is_present("invert");

    Ok(config)
}

fn find_files(files: &[String], recursive: bool) -> Vec<MyResult<String>> {
    let mut results = vec![];
    for path in files {
        match path.as_str() {
            "-" => results.push(Ok(path.to_string())),
            _ => match fs::metadata(&path) {
                Ok(metadata) => {
                    if metadata.is_dir() {
                        if recursive {
                            for entry in WalkDir::new(path)
                                .into_iter()
                                .filter_map(|e| e.ok())
                                .filter(|e| e.file_type().is_file())
                            {
                                results.push(Ok(entry.path().display().to_string()));
                            }
                        } else {
                            results.push(Err(From::from(format!("{} is a directory", path))));
                        }
                    } else if metadata.is_file() {
                        results.push(Ok(path.to_string()));
                    }
                }
                Err(e) => results.push(Err(From::from(format!("{}: {}", path, e)))),
            },
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::find_files;
    use rand::{distributions::Alphanumeric, Rng};
    #[test]
    fn test_find_files() {
        // Verify that the function finds a file known to exist
        let files = find_files(&["./tests/inputs/fox.txt".to_string()], false);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].as_ref().unwrap(), "./tests/inputs/fox.txt");
        // The function should reject a directory without the    recursive option
        let files = find_files(&["./tests/inputs".to_string()], false);
        assert_eq!(files.len(), 1);
        if let Err(e) = &files[0] {
            assert_eq!(e.to_string(), "./tests/inputs is a directory".to_string());
        }
        // Verify the function recurses to find four files in the    directory
        let res = find_files(&["./tests/inputs".to_string()], true);
        let mut files: Vec<String> = res
            .iter()
            .map(|r| r.as_ref().unwrap().replace("\\", "/"))
            .collect();
        files.sort();
        assert_eq!(files.len(), 4);
        assert_eq!(
            files,
            vec![
                "./tests/inputs/bustle.txt",
                "./tests/inputs/empty.txt",
                "./tests/inputs/fox.txt",
                "./tests/inputs/nobody.txt",
            ]
        );
        // Generate a random string to represent a nonexistent file
        let bad: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();
        // Verify that the function returns the bad file as anerror
        let files = find_files(&[bad], false);
        assert_eq!(files.len(), 1);
        assert!(files[0].is_err());
    }
}

fn find_lines<T: BufRead>(
    mut file: T,
    pattern: &Regex,
    invert_match: bool,
) -> MyResult<Vec<String>> {
    let mut matches = vec![];
    let mut line = String::new();
    loop {
        let bytes = file.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        if (pattern.is_match(&line) && !invert_match) || (!pattern.is_match(&line) && invert_match)
        {
            matches.push(line.clone());
        }
        line.clear();
    }
    Ok(matches)
}

fn open(filename: &str) -> MyResult<Box<dyn BufRead>> {
    match filename {
        "-" => Ok(Box::new(BufReader::new(io::stdin()))),
        _ => Ok(Box::new(BufReader::new(File::open(filename)?))),
    }
}

pub fn run(config: Config) -> MyResult<()> {
    let entries = find_files(&config.files, config.recursive);
    let num_files = &entries.len();
    let print = |fname: &str, val: &str| {
        if num_files > &1 {
            print!("{}:{}", fname, val);
        } else {
            print!("{}", val);
        }
    };
    for entry in entries {
        match entry {
            Err(e) => eprintln!("{}", e),
            Ok(filename) => match open(&filename) {
                Err(e) => eprintln!("{}: {}", filename, e),
                Ok(file) => match find_lines(file, &config.pattern, config.invert_match) {
                    Err(e) => eprintln!("{}", e),
                    Ok(matches) => {
                        if config.count {
                            print(&filename, &format!("{}\n", &matches.len()));
                        } else {
                            for line in &matches {
                                if !config.invert_match {
                                    let mut new_line = line.clone();
                                    let mat = config.pattern.find(&new_line).unwrap();
                                    let (init, _) = (mat.start(), ());
                                    let mut colored_text = new_line.split_off(init);
                                    let mat = config.pattern.find(&colored_text).unwrap();
                                    let (_, end) = (mat.start(), mat.end());
                                    let remainder = colored_text.split_off(end);

                                    if num_files > &1 {
                                        print!("{}: ", filename);
                                    }
                                    print!(
                                        "{}{}{}",
                                        new_line,
                                        colored_text.as_str().green(),
                                        remainder
                                    );
                                } else {
                                    print(&filename, line);
                                }
                            }
                        }
                    }
                },
            },
        }
    }
    Ok(())
}
