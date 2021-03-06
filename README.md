# markov-chain
A markov chain library for Rust.

# [Documentation](https://docs.rs/markov-chain/0.1.0/markov_chain/)

# [Crates.io](https://crates.io/crates/markov-chain)

# Features
* Training sequences of arbitrary types
* Nodes of N order
* Specialized string generation and training
* Serialization via serde
* Generation utility

# Wishlist
* Iterator for getting random items one-at-a-time
* Infinite chain generation
* Implementations of serde file writing in a utility module
* Finished documentation complete with examples

# Building
By default, the library does not build the full `markov-util` binary. To include
the binary in the build, add the flags `--features "generator serde_cbor
serde_yaml"` to `cargo build` to generate the binary. Additionally, if you do
not want to compile with CBOR or YAML support, you can remove whichever you
want to omit. **Also note that at least one serde_\* library is required to build
markov-util.**

# Basic usage
In your Cargo.toml file, make sure you have the line `markov_chain = "0.1"`
under the `[dependencies]` section.

Markov chains may be created with any type that implements `Clone`, `Hash`,
and `Eq`, and with some order (which is the number of items per node on the
markov chain).

It can be used with numbers:

```rust
use markov_chain::Chain;

let mut chain = Chain::new(1); // 1 is the order of the chain

// Train the chain on some vectors
chain.train(vec![1, 2, 3, 2, 1, 2, 3, 4, 3, 2, 1])
    .train(vec![5, 4, 3, 2, 1]);

// Generate a sequence and print it out
let sequence = chain.generate();
for number in sequence {
    print!("{} ", number);
}
println!("");
```

`Chain<T>` also derives from the serde `Serialize` and `Deserialize` traits, so
any `T` that derives those traits may be converted to and from a serialized
form. This is useful for writing to/from files. (File writing is on the TODO
list).

```rust
// TODO: file writing example
```

# License
ISC, see [COPYING](COPYING) for details.
