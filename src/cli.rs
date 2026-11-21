//! `sfs token`, `sfs hash`, `sfs keygen`: make the secrets. They print and exit; they need no settings and open no port.

use crate::keys::{new_relay_keys, new_token, token_hash};
use std::{error::Error, io::Read};

pub const USAGE: &str = "sfs                 serve the current directory (settings: environment or .env)
sfs token           make the token of a server: TOKEN for its .env, and its SHA-256 for the relay
sfs hash            print the SHA-256 of a token read from the standard input
sfs keygen          make the key pair of the relay: the private key for the relay, the public key for every server";

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    match args {
        [cmd] if cmd == "token" => {
            let token = new_token();
            println!("TOKEN={token}   # in the .env of this server (keep it secret)");
            println!(
                "SHA256={}   # in SERVERS of the relay, next to the ORIGIN of this server",
                token_hash(&token)
            );
        }
        [cmd] if cmd == "hash" => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            println!("{}", token_hash(input.trim()));
        }
        [cmd] if cmd == "keygen" => {
            let (private, public) = new_relay_keys();
            println!("RELAY_PRIVATE_KEY={private}   # only on the relay: `wrangler secret put RELAY_PRIVATE_KEY` (keep it secret)");
            println!("RELAY_PUBLIC_KEY={public}   # in the .env of every server, and in [vars] of the relay");
        }
        [cmd] if cmd == "-h" || cmd == "--help" || cmd == "help" => println!("{USAGE}"),
        _ => return Err(format!("unknown arguments\n{USAGE}").into()),
    }
    Ok(())
}
