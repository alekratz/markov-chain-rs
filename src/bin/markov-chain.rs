#[warn(missing_docs)]
#[cfg(feature = "generator")] extern crate markov_chain;
#[cfg(feature = "generator")] extern crate serde;
#[cfg(feature = "generator")] #[macro_use] extern crate clap;
#[cfg(feature = "generator")] #[macro_use] extern crate lazy_static;
#[cfg(feature = "serde_cbor")] extern crate serde_cbor as cbor;
#[cfg(feature = "serde_yaml")] extern crate serde_yaml as yaml;

mod prelude {
    #![cfg(feature = "generator")]
    lazy_static! {
        static ref FILE_EXTENSIONS: Vec<(&'static str, &'static str)> = {
            let mut extensions = Vec::new();
            if cfg!(feature = "serde_cbor") {
                extensions.push(("cbor", "CBOR, Concise Binary Object Representation"));
            }
            extensions
        };

        pub static ref AVAILABLE_FORMATS: String = {
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

    #[cfg(any(feature = "serde_cbor", feature = "serde_yaml"))]
    mod serde_strategy {
        use markov_chain::{Chain, Chainable};
        use serde::{Serialize, Deserialize};
        use std::result;
        use std::io::{self, Read, Write};
        use std::fs::{File, OpenOptions};

        pub fn read_file(path: &str) -> io::Result<Vec<u8>> {
            let mut file = File::open(path)?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)?;
            Ok(contents)
        }

        pub fn write_file(path: &str, bytes: &[u8]) -> io::Result<()> {
            let mut file = OpenOptions::new().create(true).write(true).open(path)?;
            file.write_all(bytes)
        }

        type Result<T> = result::Result<T, String>;

        pub enum SerdeStrategy {
            CBOR,
            Yaml,
        }

        impl SerdeStrategy {
            pub fn from_path(path: &str) -> Option<SerdeStrategy> {
                if cfg!(feature = "serde_cbor") && path.ends_with(".cbor") {
                    Some(SerdeStrategy::CBOR)
                }
                else if cfg!(feature = "serde_yaml") && path.ends_with(".yaml") {
                    Some(SerdeStrategy::Yaml)
                }
                else {
                    None
                }
            }

            pub fn into_vec<T>(self, chain: &Chain<T>) -> Result<Vec<u8>>
                where for<'de> T: Chainable + Clone + Serialize + Deserialize<'de> {
                use self::SerdeStrategy::*;
                match self {
                    CBOR => Self::to_cbor(chain),
                    Yaml => Self::to_yaml(chain),
                }
            }

            pub fn from_slice<T>(self, slice: &[u8]) -> Result<Chain<T>> 
                where for<'de> T: Chainable + Clone + Serialize + Deserialize<'de> {
                use self::SerdeStrategy::*;
                match self {
                    CBOR => Self::from_cbor(slice),
                    Yaml => Self::from_yaml(slice),
                }
            }

            #[cfg(feature = "serde_cbor")]
            pub fn to_cbor<T>(chain: &Chain<T>) -> Result<Vec<u8>>
                where for<'de> T: Chainable + Clone + Serialize + Deserialize<'de> {
                use cbor;
                cbor::to_vec(chain).map_err(|e| e.to_string())
            }

            #[cfg(feature = "serde_cbor")]
            pub fn from_cbor<T>(slice: &[u8]) -> Result<Chain<T>>
                where for<'de> T: Chainable + Clone + Serialize + Deserialize<'de> {
                use cbor;
                cbor::from_slice(slice).map_err(|e| e.to_string())
            }

            #[cfg(feature = "serde_yaml")]
            pub fn to_yaml<T>(chain: &Chain<T>) -> Result<Vec<u8>>
                where for<'de> T: Chainable + Clone + Serialize + Deserialize<'de> {
                use yaml;
                yaml::to_string(chain).map(|c| c.into_bytes()).map_err(|e| e.to_string())
            }

            #[cfg(feature = "serde_yaml")]
            pub fn from_yaml<T>(slice: &[u8]) -> Result<Chain<T>>
                where for<'de> T: Chainable + Clone + Serialize + Deserialize<'de> {
                use std::str;
                use yaml;
                yaml::from_str(str::from_utf8(slice).unwrap()).map_err(|e| e.to_string())
            } 
        }


        pub fn write_chain<T>(chain: &Chain<T>, path: &str) -> Result<()>
            where for<'de> T: Chainable + Clone + Serialize + Deserialize<'de> {
            if let Some(strat) = SerdeStrategy::from_path(path) {
                let bytes: Vec<u8> = strat.into_vec(chain)?;
                write_file(path, &bytes).map_err(|e| e.to_string())
            }
            else {
                Err(format!("unknown strategy for writing chain file `{}`", path))
            }
        }

        pub fn read_chain<T>(path: &str) -> Result<Chain<T>>
            where for<'de> T: Chainable + Clone + Serialize + Deserialize<'de> {
            if let Some(strat) = SerdeStrategy::from_path(path) {
                let bytes = match read_file(path) {
                    Ok(b) => b,
                    Err(e) => return Err(e.to_string())
                };
                strat.from_slice(&bytes).map_err(|e| e.to_string())
            }
            else {
                Err(format!("unknown strategy for reading chain file `{}`", path))
            }
        }
    }

    #[cfg(any(feature = "serde_cbor", feature = "serde_yaml"))]
    use self::serde_strategy::*;


    use markov_chain::Chain;
    use std::io::{self, Write};
    use std::process;
    use std::fmt::Display;
    use std::path::Path;

    macro_rules! exit_err {
        ($fmt:expr, $( $item:expr ),*) => {
            exit_err(format!($fmt, $($item),*));
        };
    } 

    pub fn train(order: usize, update_files: Vec<&str>, input_files: Vec<&str>) {
        let mut chains = Vec::new();

        // make sure all the input files exist
        for input in &input_files {
            if !Path::new(input).exists() {
                exit_err!("could not find input file `{}`", input);
            }
        }

        // convert the update files into chains
        for update in update_files {
            let update_path = Path::new(update);
            if update_path.exists() {
                let chain = match read_chain(update) {
                    Ok(c) => c,
                    Err(e) => exit_err!("{}", e),
                };
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
            if let Err(e) = write_chain(&chain, path) {
                let mut stderr = io::stderr();
                writeln!(stderr, "could not write {}: {}", path, e).unwrap();
            }
        }
    }

    pub fn generate(order: usize, paragraphs: usize, sentences: usize, input_files: Vec<&str>) {
        let mut chain = Chain::<String>::new(order);
        for input in input_files {
            if SerdeStrategy::from_path(input).is_some() {
                let input_chain = match read_chain(input) {
                    Ok(c) => c,
                    Err(e) => exit_err!("could not read {}: {}", input, e),
                };
                chain.merge(&input_chain);
            }
            else {
                let contents = match read_file(input) {
                    Ok(c) => String::from_utf8(c).unwrap(),
                    Err(e) => exit_err!("could not read {}: {}", input, e),
                };
                chain.train_string(&contents);
            };
        }
        let mut pgs = Vec::new();
        // generate paragraphs
        for _ in 0 .. paragraphs {
            pgs.push(chain.generate_paragraph(sentences));
        }
        println!("{}", pgs.join("\n\n"));
    }

    pub fn merge(order: usize, input_files: Vec<&str>, output_file: &str) {
        if SerdeStrategy::from_path(output_file).is_none() {
            exit_err!("unknown strategy for writing {}", output_file);
        }

        let mut chain = Chain::<String>::new(order);
        for input in input_files {
            if SerdeStrategy::from_path(input).is_some() {
                let input_chain = match read_chain(input) {
                    Ok(c) => c,
                    Err(e) => exit_err!("could not read {}: {}", input, e),
                };
                chain.merge(&input_chain);
            }
            else {
                let contents = match read_file(input) {
                    Ok(c) => String::from_utf8(c).unwrap(),
                    Err(e) => exit_err!("could not read {}: {}", input, e),
                };
                chain.train_string(&contents);
            };
        }
        
        if let Err(e) = write_chain(&chain, output_file) {
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
use prelude::*;

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
