//! A markov chain library for Rust.
//! 
//! # Features
//! * Training sequences of arbitrary types
//! * Nodes of N order
//! * Specialized string generation and training
//! * Serialization via serde
//! * Generation utility
//! 
//! # Examples
//! In your Cargo.toml file, make sure you have the line `markov_chain = "0.1"`
//! under the `[dependencies]` section.
//! 
//! Markov chains may be created with any type that implements `Clone`, `Hash`,
//! and `Eq`, and with some order (which is the number of items per node on the
//! markov chain).
//! 
//! ## Creating a basic chain
//!
//! ```
//! use markov_chain::Chain;
//! 
//! let mut chain = Chain::new(1); // 1 is the order of the chain
//! 
//! // Train the chain on some vectors
//! chain.train(vec![1, 2, 3, 2, 1, 2, 3, 4, 3, 2, 1])
//!     .train(vec![5, 4, 3, 2, 1]);
//! 
//! // Generate a sequence and print it out
//! let sequence = chain.generate();
//! println!("{:?} ", sequence);
//! ```
#![warn(missing_docs)]
extern crate serde;
#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate maplit;
extern crate rand;
extern crate regex;

#[macro_use]
extern crate lazy_static;

use rand::distributions::{Weighted, WeightedChoice, IndependentSample};
use rand::Rng;
use regex::Regex;
use std::collections::HashMap;
use std::hash::Hash;

// Stolen from public domain project https://github.com/aatxe/markov
/// A trait that defines a restrictions required for chainable items.
pub trait Chainable: Eq + Hash {}
impl<T> Chainable for T where T: Eq + Hash {}

type Node<T> = Vec<Option<T>>;
type Link<T> = HashMap<Option<T>, u32>;

// don't add where T: Serialize + DeserializeOwned, see
// https://github.com/serde-rs/serde/issues/890
/// A struct representing a markov chain.
///
/// A markov chain has an order, which determines how many items
/// per node are held. The chain itself is a map of vectors, which point to
/// a map of single elements pointing at a weight.
///
/// ```
/// use markov_chain::Chain;
/// 
/// let mut chain = Chain::new(1); // 1 is the order of the chain
/// 
/// // Train the chain on some vectors
/// chain.train(vec![1, 2, 3, 2, 1, 2, 3, 4, 3, 2, 1])
///     .train(vec![5, 4, 3, 2, 1]);
/// 
/// // Generate a sequence and print it out
/// let sequence = chain.generate();
/// println!("{:?} ", sequence);
/// ```
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Chain<T> where T: Clone + Chainable {
    chain: HashMap<Node<T>, Link<T>>,
    order: usize,
}

impl<T> Chain<T> where T: Clone + Chainable {
    /// Initializes a new markov chain with a given order.
    /// # Examples
    /// ```
    /// use markov_chain::Chain;
    /// let chain: Chain<u32> = Chain::new(1);
    /// ```
    pub fn new(order: usize) -> Self {
        Chain {
            chain: HashMap::new(),
            order,
        }
    } 

    /// Gets the order of the markov chain. This is static from chain to chain.
    pub fn order(&self) -> usize {
        self.order
    }

    /// Gets whether the internal markov chain is empty.
    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }

    /// Gets the raw underlying chain data structure.
    pub fn chain(&self) -> &HashMap<Node<T>, Link<T>> {
        &self.chain
    }

    /// Trains a sentence on a string of items.
    /// # Examples
    /// ```
    /// use markov_chain::Chain;
    /// let mut chain = Chain::new(1);
    /// let data = vec![10, 15, 20];
    /// chain.train(data)
    ///     .train(vec![]);
    /// ```
    pub fn train(&mut self, string: Vec<T>) -> &mut Self {
        if string.is_empty() {
            return self;
        }

        let order = self.order;

        let mut string = string.into_iter()
            .map(|x| Some(x))
            .collect::<Vec<Option<T>>>();
        while string.len() < order {
            string.push(None);
        }

        let mut window = vec!(None; order);
        self.update_link(&window, &string[0]);

        let mut end = 0;
        while end < string.len() - 1 {
            window.remove(0);
            let next = &string[end + 1];
            window.push(string[end].clone());

            self.update_link(&window, &next);

            end += 1;
        }
        window.remove(0);
        window.push(string[end].clone());
        self.update_link(&window, &None);
        self
    }

    /// Merges this markov chain with another.
    /// # Examples
    /// ```
    /// use markov_chain::Chain;
    /// let mut chain1 = Chain::new(1);
    /// let mut chain2 = chain1.clone();
    /// chain1.train(vec![1, 2, 3]);
    /// chain2.train(vec![2, 3, 4, 5, 6])
    ///     .merge(&chain1);
    /// ```
    pub fn merge(&mut self, other: &Self) -> &mut Self {
        assert_eq!(self.order, other.order, "orders must be equal in order to merge markov chains");
        if self.chain.is_empty() {
            self.chain = other.chain.clone();
            return self;
        }

        for (node, link) in &other.chain {
            for (ref next, &weight) in link.iter() {
                self.update_link_weight(node, next, weight);
            }
        }
        self
    }

    /// Increments a link from a node by one, or adding it with a weight of 1
    /// if it doesn't exist.
    fn update_link(&mut self, node: &[Option<T>], next: &Option<T>) {
        self.update_link_weight(node, next, 1);
    }

    /// Increments a link from a node by specified value, or adding it with a
    /// weight of the specified value if it doesn't exist.
    fn update_link_weight(&mut self, node: &[Option<T>], next: &Option<T>, weight: u32) {
        if self.chain.contains_key(node) {
            let links = self.chain
                .get_mut(node)
                .unwrap();
            // Update the link
            if links.contains_key(next) {
                let weight = *links.get(next).unwrap() + weight;
                links.insert(next.clone(), weight);
            }
            // Insert a new link
            else {
                links.insert(next.clone(), weight);
            }
        }
        else {
            self.chain.insert(Vec::from(node), hashmap!{next.clone() => weight});
        }
    }

    /// Generates a string of items with no maximum limit.
    /// This is equivalent to `generate_limit(-1)`.
    pub fn generate(&self) -> Vec<T> {
        self.generate_limit(-1)
    }

    /// Generates a string of items, based on the training, of up to N items.
    /// Specifying a maximum of -1 allows any arbitrary size of list.
    pub fn generate_limit(&self, max: isize) -> Vec<T> {
        // TODO : DRY generate_sentence(1)
        if self.chain.is_empty() {
            return vec![];
        }

        let mut curs = {
            let c;
            loop {
                if let Some(n) = self.choose_random_node() {
                    c = n.clone();
                    break;
                }
            }
            c
        };

        // this takes care of an instance where we have order N and have chosen a node that is
        // shorter than our order.
        if curs.iter().find(|x| x.is_none()).is_some() {
            return curs.iter()
                .cloned()
                .filter_map(|x| x)
                .collect();
        }

        let mut result = curs.clone()
            .into_iter()
            .map(|x| x.unwrap())
            .collect::<Vec<T>>();

        loop {
            // Choose the next item
            let next = self.choose_random_link(&curs);
            if let Some(next) = next {
                result.push(next.clone());
                curs.push(Some(next.clone()));
                curs.remove(0);
            }
            else {
                break;
            }

            if result.len() as isize >= max && max > 0 {
                break;
            }
        }
        result
    }

    fn choose_random_link(&self, node: &Node<T>) -> Option<&T> {
        assert_eq!(node.len(), self.order);
        if let Some(ref link) = self.chain.get(node) {
            let mut weights = link.iter()
                .map(|(k, v)| Weighted { weight: *v, item: k.as_ref() })
                .collect::<Vec<_>>();
            let chooser = WeightedChoice::new(&mut weights);
            let mut rng = rand::thread_rng();
            chooser.ind_sample(&mut rng)
        }
        else {
            None
        }
    }

    fn choose_random_node(&self) -> Option<&Node<T>> {
        if self.chain.is_empty() {
            None
        }
        else {
            let mut rng = rand::thread_rng();
            self.chain.keys()
                .nth(rng.gen_range(0, self.chain.len()))
        }
    }
}

lazy_static! { 
    /// Symbol combinations to break sentences on.
    static ref BREAK: [&'static str; 7] = [".", "?", "!", ".\"", "!\"", "?\"", ",\""];
}
/// String-specific implementation of the chain. Contains some special string-
/// specific functions.
impl Chain<String> {
    /// Trains this chain on a single string. Strings are broken into words,
    /// which are split by whitespace and punctuation.
    pub fn train_string(&mut self, sentence: &str) -> &mut Self {
        lazy_static! {
            static ref RE: Regex = Regex::new(
                r#"[^ .!?,\-\n\r\t]+|[.,!?\-"]+"#
                ).unwrap();
        };
        let parts = {
            let mut parts = Vec::new();
            let mut words = Vec::new();
            for mat in RE.find_iter(sentence).map(|m| m.as_str()) {
                words.push(String::from(mat));
                if BREAK.contains(&mat) {
                    parts.push(words.clone());
                    words.clear();
                }
            }
            if !words.is_empty() {
                parts.push(words);
            }
            parts
        };
        for string in parts {
            self.train(string);
        }
        self
    }

    /// Generates a sentence, which are ended by "break" strings or null links.
    /// "Break" strings are:
    /// `.`, `?`, `!`, `."`, `!"`, `?"`, `,"`
    pub fn generate_sentence(&self) -> String {
        // TODO : DRY generate_sentence(1)
        // consider an iterator?
        if self.chain.is_empty() {
            return String::new();
        }

        let mut curs = vec!(None; self.order);
        let mut result = Vec::new();
        loop {
            // Choose the next item
            let next = self.choose_random_link(&curs);
            if let Some(next) = next {
                result.push(next.clone());
                curs.push(Some(next.clone()));
                curs.remove(0);
                if BREAK.contains(&next.as_str()) {
                    break;
                }
            }
            else {
                break;
            }
        }
        let mut result = result.into_iter()
            .fold(String::new(), |a, b| if BREAK.contains(&b.as_str()) || b == "," { a + b.as_str() } else { a + " " + b.as_str() });
        result.remove(0); // get rid of the leading space character
        result
    }

    /// Generates a paragraph of N sentences. Each sentence is broken off by N
    /// spaces.
    pub fn generate_paragraph(&self, sentences: usize) -> String {
        let mut paragraph = Vec::new();
        for _ in 0 .. sentences {
            paragraph.push(self.generate_sentence());
        }
        paragraph.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use ::*;

    macro_rules! test_get_link {
        ($chain:expr, [$($key:expr),+]) => {{
            let ref map = $chain.chain;
            let key = vec![$(Some($key),)+];
            assert_eq!(key.len(), $chain.order);
            assert!(map.contains_key(&key));
            map.get(&key)
                .unwrap()
        }}; 
    }

    macro_rules! test_link_weight {
        ($link:expr, $key:expr, $weight:expr) => {
            let link = $link;
            let key = $key;
            assert!(link.contains_key(&key));
            assert_eq!(*link.get(&key).unwrap(), $weight);
        };
    }

    #[cfg(feature = "serde_cbor")]
    #[test]
    fn test_cbor_serialize() {
        let mut chain = Chain::<u32>::new(1);
        chain.train(vec![1, 2, 3])
            .train(vec![2, 3, 4])
            .train(vec![1, 3, 4]);
        let cbor_vec = chain.to_cbor();
        assert!(cbor_vec.is_ok());
        let de = Chain::from_cbor(&cbor_vec.unwrap());
        assert_eq!(de.unwrap(), chain);
    }

    #[cfg(feature = "serde_yaml")]
    #[test]
    fn test_yaml_serialize() {
        let mut chain = Chain::<u32>::new(1);
        chain.train(vec![1, 2, 3])
            .train(vec![2, 3, 4])
            .train(vec![1, 3, 4]);
        let yaml_str = chain.to_yaml();
        assert!(yaml_str.is_ok());
        let de = Chain::from_yaml(&yaml_str.unwrap());
        assert_eq!(de.unwrap(), chain);
    }

    #[test]
    fn test_order1_training() {
        let mut chain = Chain::<u32>::new(1);
        chain.train(vec![1, 2, 3])
            .train(vec![2, 3, 4])
            .train(vec![1, 3, 4]);
        let link = test_get_link!(chain, [1u32]);
        test_link_weight!(link, Some(2u32), 1);
        test_link_weight!(link, Some(3u32), 1);

        let link = test_get_link!(chain, [2u32]);
        test_link_weight!(link, Some(3u32), 2);

        let link = test_get_link!(chain, [3u32]);
        test_link_weight!(link, None, 1);
        test_link_weight!(link, Some(4u32), 2);

        let link = test_get_link!(chain, [4u32]);
        test_link_weight!(link, None, 2);
    }

    #[test]
    fn test_order2_training() {
        let mut chain = Chain::<u32>::new(2);
        chain.train(vec![1, 2, 3])
            .train(vec![2, 3, 4])
            .train(vec![1, 3, 4]);
        let link = test_get_link!(chain, [1u32, 2u32]);
        test_link_weight!(link, Some(3u32), 1);

        let link = test_get_link!(chain, [2u32, 3u32]);
        test_link_weight!(link, None, 1);
        test_link_weight!(link, Some(4u32), 1);

        let link = test_get_link!(chain, [3u32, 4u32]);
        test_link_weight!(link, None, 2);

        let link = test_get_link!(chain, [1u32, 3u32]);
        test_link_weight!(link, Some(4u32), 1);
    }

    #[test]
    fn test_order3_training() {
        let mut chain = Chain::<u32>::new(3);
        chain.train(vec![1, 2, 3, 4, 1, 2, 3, 4]);

        let link = test_get_link!(chain, [1u32, 2u32, 3u32]);
        test_link_weight!(link, Some(4u32), 2);

        let link = test_get_link!(chain, [2u32, 3u32, 4u32]);
        test_link_weight!(link, Some(1u32), 1);
        test_link_weight!(link, None, 1);

        let link = test_get_link!(chain, [3u32, 4u32, 1u32]);
        test_link_weight!(link, Some(2u32), 1);

        let link = test_get_link!(chain, [4u32, 1u32, 2u32]);
        test_link_weight!(link, Some(3u32), 1);
    }
}
