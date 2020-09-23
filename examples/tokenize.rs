use std::env;
use std::fs;
use ruby_parser::{Lexer, SymbolKind, Token};
use std::convert::TryFrom;

fn print_usage() -> ! {
    println!("
USAGE:
    cargo run --example tokenize -- test.rb
    cargo run --example tokenize -- -e \"2 + 2\"
");
    std::process::exit(1)
}

fn token_name(token: &Token) -> String {
    let (id, _, _) = token;
    SymbolKind::get(usize::try_from(id.clone()).unwrap()).name()
}

fn token_value(token: &Token) -> String {
    let (_, value, _) = token;
    value.clone()
}

fn rpad<T: Sized + std::fmt::Debug>(value: &T, total_width: usize) -> String {
    format!("{:width$}", format!("{:?}, ", value), width = total_width)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let args: Vec<&str> = args.iter().skip(1).map(|e| &e[..]).collect();

    let source =
        match args[..] {
            ["-e", code] => code.to_owned(),
            [filepath] => fs::read_to_string(filepath).expect("Failed to read file"),
            _ => print_usage()
        };

    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize_until_eof();

    let tok_name_length  = tokens.iter().map(|tok| format!("{:?}", token_name(tok)).len()).max().unwrap_or(0) + 2;
    let tok_value_length = tokens.iter().map(|tok| format!("{:?}", token_value(tok)).len()).max().unwrap_or(0) + 2;

    println!("[");
    for token in tokens {
        let (_, _, loc) = &token;
        let name = rpad(&token_name(&token), tok_name_length);
        let value = rpad(&token_value(&token), tok_value_length);
        println!("    :{}{}[{}, {}]", name, value, loc.begin, loc.end);
    }
    println!("]");
}
