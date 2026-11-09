//! Stand-in server for the serving test: prints each argument it receives on its own line.

fn main() {
    for argument in std::env::args_os().skip(1) {
        println!("{}", argument.to_string_lossy());
    }
}
