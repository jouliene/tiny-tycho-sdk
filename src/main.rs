use anyhow::Result;
use tiny_tycho_sdk::{JrpcTransport, Keys};

#[tokio::main]
async fn main() -> Result<()> {
    // Keys generation test module
    let phrase = "alter sustain pulp catalog announce tail bunker mammal figure burger party title";
    let keys_0 = Keys::from_seed_phrase(phrase)?; //use index=0 by default
    println!("Public 0: {}", keys_0.public_key_hex());
    println!("Secret 0: {}", keys_0.secret_key_hex());

    let keys_1 = Keys::from_seed_phrase_with_index(phrase, 1)?;
    println!("Public 1: {}", keys_1.public_key_hex());
    println!("Secret 1: {}", keys_1.secret_key_hex());

    let new_seed = Keys::generate_seed_phrase()?;
    println!("New Seed: {}", new_seed);
    let new_keys = Keys::from_seed_phrase(&new_seed)?;
    println!("Public new: {}", new_keys.public_key_hex());
    println!("Secret new: {}", new_keys.secret_key_hex());

    let gen_keys = Keys::generate_keys();
    println!("Public generated: {}", gen_keys.public_key_hex());
    println!("Secret generated: {}", gen_keys.secret_key_hex());

    let der_key = Keys::from_secret_hex_str(
        "f97ca5343717b0ea2f2234562ceb0e5cc53b7eb7a2519385049a736f78e51432",
    )?;
    println!("Public derived: {}", der_key.public_key_hex());
    println!("Secret derived: {}", der_key.secret_key_hex());

    // Jrpc test module
    let transport = JrpcTransport::new("https://rpc-testnet.tychoprotocol.com")?;
    let caps = transport.get_capabilities().await?;
    println!("Capabilities: {caps:#?}");

    let sig_ctx = transport.get_signature_context().await?;
    println!("global_id: {}", sig_ctx.global_id);
    println!("capabilities: {:?}", sig_ctx.capabilities);

    Ok(())
}
