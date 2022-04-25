Symbolic execution engine that operates on LLVM IR. Main use case is to analyze Rust programs but
as it operates on LLVM IR it allows it to analyze all software that can generate LLVM IR.

## Getting started

The project currently uses LLVM 13 so the newest Rust version that can be used is `1.59`.

To get the tests and the C examples to run the bitcode files have to be generated, these can be
generated by running

```sh
scripts/compile_samples.sh
scripts/compile_tests.sh
```


## Cargo subcommand

A cargo subcommand is available to easily compile Rust programs into bitcode files and run them
in the symbolic execution engine.

It can be installed with

```sh
cargo install --path cli
```

To compile and run the example `examples/rust_simple`

```sh
cargo x0001e --example rust_simple --function rust_simple_test
```

This will display the results of the analysis of the example, showing all the paths it took and
concrete values for all inputs and output.


## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
