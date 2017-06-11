#[warn(missing_docs)]
#[cfg(feature = "generator")] extern crate markov_chain;
#[cfg(feature = "generator")] #[macro_use] extern crate clap;
#[cfg(feature = "generator")] #[macro_use] extern crate lazy_static;

#[cfg(feature = "generator")]
lazy_static! {
    static ref FILE_EXTENSIONS: Vec<(&'static str, &'static str)> = {
        let mut extensions = Vec::new();
        if cfg!(feature = "serde_cbor") {
            extensions.push(("cbor", "CBOR, Concise Binary Object Representation"));
        }
        extensions
    };

    static ref AVAILABLE_FORMATS: String = {
        let mut available_formats = String::from(
r#"The file format of the chains to train is determined by its file extension.
These are the file formats and extensions supported:

"#);
        let max = FILE_EXTENSIONS.iter()
            .map(|&(x, _)| x.len())
            .fold(0, |a, b| if a > b { a } else { b }) + 4;
        for &(ext, desc) in FILE_EXTENSIONS.iter() {
            available_formats += format!("{1:>0$} - {2}\n", max, format!(".{}", ext), desc).as_str();
        }
        available_formats
    };
}

mod deps {
    #![cfg(feature = "generator")]

    use ::FILE_EXTENSIONS;
    use markov_chain::Chain;
    use std::io::{self, Write, Read};
    use std::process;
    use std::fmt::Display;
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::path::Path;

    macro_rules! exit_err {
        ($fmt:expr, $( $item:expr ),*) => {
            exit_err(format!($fmt, $($item),*));
        };
    }

    fn read_file(path: &str) -> io::Result<Vec<u8>> {
        let mut file = File::open(path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        Ok(contents)
    }

    fn write_file(path: &str, bytes: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new().create(true).write(true).open(path)?;
        file.write_all(bytes)
    }

    pub fn is_valid_extension(ext: &str) -> bool {
        FILE_EXTENSIONS.iter()
            .find(|x| x.0 == ext)
            .is_some()
    }

    pub fn train(order: usize, update_files: Vec<&str>, input_files: Vec<&str>) {
        let mut chains = Vec::new();

        // make sure all the input files exist
        for input in &input_files {
            if !Path::new(input).exists() {
                exit_err!("could not find input file `{}`", input);
            }
        }

        // make sure all chain files have known extensions
        for update in &update_files {
            // if someone wants to DRY this loop that'd be great
            if let Some(extension) = Path::new(update).extension() {
                if !is_valid_extension(extension.to_str().unwrap()) {
                    exit_err!("no known strategy to read file `{}`. Known extensions: {}",
                              update,
                              FILE_EXTENSIONS.iter().map(|&(a,_)| a).collect::<Vec<&str>>().join(" "));
                }
            }
            else {
                exit_err!("no known strategy to read file `{}`. Known extensions: {}",
                          update,
                          FILE_EXTENSIONS.iter().map(|&(a,_)| a).collect::<Vec<&str>>().join(" "));
            }
        }

        // convert the update files into chains
        for update in update_files {
            let update_path = Path::new(update);
            if update_path.exists() {
                println!("Loading {}", update);
                let contents = match read_file(update) {
                    Ok(c) => c,
                    Err(e) => exit_err!("error reading {}: {}", update, e),
                };
                // choose chain strategy
                let chain = if update.ends_with(".cbor") {
                    match Chain::<String>::from_cbor(&contents) {
                        Ok(c) => c,
                        Err(e) => exit_err!("could not read cbor file: {}", e),
                    }
                }
                else {
                    unreachable!()
                };
                if chain.order() != order {
                    exit_err!("chain file `{}` has a chain with order {}, but {} was specified on the command line",
                              update, chain.order(), order);
                }
                chains.push((update, chain));
            }
            else {
                println!("{} does not exist, it will be created", update);
                chains.push((update, Chain::new(order)));
            }
        }

        // read each input file
        let mut inputs = Vec::new();
        for input in &input_files {
            let contents = match read_file(input) {
                Ok(c) => String::from_utf8(c).unwrap(),
                Err(e) => exit_err!("could not read `{}`: {}", input, e),
            };
            inputs.push(contents);
        }

        // train and write
        for (path, mut chain) in chains {
            println!("Training {}", path);
            for input in &inputs {
                chain.train_string(input);
            }

            println!("Writing {}", path);
            let write_bytes = match Path::new(path).extension().map(|x| x.to_str().unwrap()) {
                Some("cbor") => chain.to_cbor().unwrap(),
                _ => unreachable!(),
            };

            if let Err(e) = write_file(path, &write_bytes) {
                let mut stderr = io::stderr();
                writeln!(stderr, "Error writing to {}: {}", path, e).unwrap();
            }
        }
    }

    pub fn generate(order: usize, paragraphs: usize, sentences: usize, input_files: Vec<&str>) {
        let mut chain = Chain::<String>::new(order);
        for input in input_files {
            let contents = match read_file(input) {
                Ok(c) => c,
                Err(e) => exit_err!("could not read {}: {}", input, e),
            };

            // train the chain based on the extension
            if let Some(extension) = Path::new(input).extension().map(|x| x.to_str().unwrap()) {
                if is_valid_extension(extension) {
                    match extension {
                        "cbor" => match Chain::<String>::from_cbor(&contents) {
                            Ok(c) => if c.order() != order {
                                exit_err!("could not load chain file {0}: {0} has an order of {1}, while {2} is specified",
                                          input, c.order(), order);
                            }
                            else {
                                chain.merge(&c);
                            },
                            Err(e) => exit_err!("could not parse cbor file {}: {}", input, e),
                        },
                        _ => unreachable!(),
                    }
                }
                else {
                    // TODO : DRY generate(1)
                    match String::from_utf8(contents) {
                        Ok(contents) => chain.train_string(&contents),
                        Err(e) => exit_err!("error reading {} as plaintext: {}", input, e),
                    };
                }
            }
            else {
                    // TODO : DRY generate(1)
                match String::from_utf8(contents) {
                    Ok(contents) => chain.train_string(&contents),
                    Err(e) => exit_err!("error reading {} as plaintext: {}", input, e),
                };
            }

        }
        let mut pgs = Vec::new();
        // generate paragraphs
        for _ in 0 .. paragraphs {
            pgs.push(chain.generate_paragraph(sentences));
        }
        println!("{}", pgs.join("\n\n"));
    }

    pub fn merge(order: usize, input_files: Vec<&str>, output_file: &str) {
        let mut chain = Chain::<String>::new(order);
        if let Some(extension) = Path::new(output_file).extension().map(|x| x.to_str().unwrap()) {
            if !is_valid_extension(extension) {
                exit_err!("no known strategy to write file `{}`. Known extensions: {}",
                          output_file,
                          FILE_EXTENSIONS.iter().map(|&(a,_)| a).collect::<Vec<&str>>().join(" "));
            }
        }
        for input in input_files {
            let contents = match read_file(input) {
                Ok(c) => c,
                Err(e) => exit_err!("could not read {}: {}", input, e),
            };

            // train the chain based on the extension
            if let Some(extension) = Path::new(input).extension().map(|x| x.to_str().unwrap()) {
                if is_valid_extension(extension) {
                    match extension {
                        "cbor" => match Chain::<String>::from_cbor(&contents) {
                            Ok(c) => if c.order() != order {
                                exit_err!("could not load chain file {0}: {0} has an order of {1}, while {2} is specified",
                                          input, c.order(), order);
                            }
                            else {
                                chain.merge(&c);
                            },
                            Err(e) => exit_err!("could not parse cbor file {}: {}", input, e),
                        },
                        _ => unreachable!(),
                    }
                }
                else {
                    // TODO : DRY generate(1)
                    match String::from_utf8(contents) {
                        Ok(contents) => chain.train_string(&contents),
                        Err(e) => exit_err!("error reading {} as plaintext: {}", input, e),
                    };
                }
            }
            else {
                // TODO : DRY generate(1)
                match String::from_utf8(contents) {
                    Ok(contents) => chain.train_string(&contents),
                    Err(e) => exit_err!("error reading {} as plaintext: {}", input, e),
                };
            }
        }
        
        let write_bytes = match Path::new(output_file).extension().map(|x| x.to_str().unwrap()).unwrap() {
            "cbor" => chain.to_cbor().unwrap(),
            _ => unreachable!(),
        };

        if let Err(e) = write_file(output_file, &write_bytes) {
            exit_err!("could not write file {}: {}", output_file, e);
        }
    }

    pub fn exit_err<T: Display>(msg: T) -> ! {
        let mut stderr = io::stderr();
        writeln!(stderr, "Error: {}", msg).unwrap();
        process::exit(1);
    }
}

#[cfg(feature = "generator")]
use deps::*;

#[cfg(feature = "generator")]
fn main() {
    let app = clap_app!(markov_generator =>
        (name: crate_name!())
        (version: crate_version!())
        (author: crate_authors!())
        (about: "A markov chain generator.")
        (after_help: AVAILABLE_FORMATS.as_str())
        (@subcommand train =>
            (about: "Trains a new markov chain, or updates an existing markov chain from a file.")
            (@arg INPUT: +required +multiple "Sets the input training data to use")
            (@arg OUTPUT: -o --output +required +takes_value +multiple "Sets the list of files to update or create")
            (@arg ORDER: -r --order +takes_value "Sets the order of the markov chain")
        )
        (@subcommand generate =>
            (about: "Generates a string of text based on a file, or a saved markov chain in a supported format.")
            (@arg INPUT: +required +multiple "Sets the input training data or markov chain file to use")
            (@arg PARAGRAPHS: -p --paragraphs +takes_value "The number of paragraphs to generate")
            (@arg SENTENCES: -s --sentences +takes_value "The number of sentences to generate per paragraph")
            (@arg ORDER: -r --order +takes_value "Sets the order of the markov chain")
        )
        (@subcommand merge =>
            (about: "Merges many markov chain files together into one file.")
            (@arg INPUT: +required +multiple "Sets the input training data or markov chain file to use")
            (@arg OUTPUT: -o --out +required +takes_value "Sets the file where the final merged markov chain is saved.")
            (@arg ORDER: -r --order +takes_value "Sets the order of the markov chain")
        )
    );
    
    let mut helper = app.clone();
    let matches = app.get_matches();

    match matches.subcommand_name() {
        Some("train") => {
            let matches = matches.subcommand_matches("train").unwrap();
            let order = match matches.value_of("ORDER")
                .map(|x| x.parse::<usize>())
                .unwrap_or(Ok(1)) {
                    Ok(n) => n,
                    Err(e) => exit_err(format!("invalid number for order: {}", e)),
                };
            if order == 0 {
                exit_err("order must be at least 1");
            }
            
            let update_files = matches.values_of("OUTPUT")
                .map(|x| x.collect())
                .unwrap_or(vec![]);
            let input_files = matches.values_of("INPUT")
                .unwrap()
                .collect();
            train(order, update_files, input_files);
        },
        Some("generate") => {
            let matches = matches.subcommand_matches("generate").unwrap();
            let order = match matches.value_of("ORDER")
                .map(|x| x.parse::<usize>())
                .unwrap_or(Ok(1)) {
                    Ok(n) => n,
                    Err(e) => exit_err(format!("invalid number for order: {}", e)),
                };
            if order == 0 {
                exit_err("order must be at least 1");
            }
            let paragraphs = match matches.value_of("PARAGRAPHS")
                .map(|x| x.parse::<usize>())
                .unwrap_or(Ok(1)) {
                    Ok(n) => n,
                    Err(e) => exit_err(format!("invalid number for paragraphs: {}", e)),
                };
            let sentences = match matches.value_of("SENTENCES")
                .map(|x| x.parse::<usize>())
                .unwrap_or(Ok(3)) {
                    Ok(n) => n,
                    Err(e) => exit_err(format!("invalid number for sentences: {}", e)),
                };
            let input_files = matches.values_of("INPUT")
                .unwrap()
                .collect();
            generate(order, paragraphs, sentences, input_files);
        },
        Some("merge") => {
            let matches = matches.subcommand_matches("merge").unwrap();
            let order = match matches.value_of("ORDER")
                .map(|x| x.parse::<usize>())
                .unwrap_or(Ok(1)) {
                    Ok(n) => n,
                    Err(e) => exit_err(format!("invalid number for order: {}", e)),
                };
            let input_files = matches.values_of("INPUT")
                .unwrap()
                .collect();
            let output_file = matches.value_of("OUTPUT")
                .unwrap();
            merge(order, input_files, output_file);
        }
        Some(command) => {
            helper.print_help().unwrap();
            println!();
            exit_err(format!("unknown command {}", command));
        },
        None => {
            helper.print_help().unwrap();
            println!();
            exit_err("command not specified");
        },
    }
}

#[cfg(not(feature = "generator"))]
fn main() {
    println!("build with --feature \"clap\" to run the markov chain generator");
}
