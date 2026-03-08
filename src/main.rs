use anyhow::Result;
use std::str::FromStr;
use tiny_tycho_sdk::{EverWallet, JrpcTransport, Keys, StdAddr};

#[tokio::main]
async fn main() -> Result<()> {
    println!("==============================");
    println!("Keys generation test module");
    println!("==============================");

    let phrase = "alter sustain pulp catalog announce tail bunker mammal figure burger party title";

    let keys_0 = Keys::from_seed_phrase(phrase)?; // default index = 0
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

    println!();
    println!("==============================");
    println!("JRPC test module");
    println!("==============================");

    let transport = JrpcTransport::new("https://rpc-testnet.tychoprotocol.com")?;
    //let transport = JrpcTransport::new("https://ppi-rpc.broxus.com")?;

    let caps = transport.get_capabilities().await?;
    println!("Capabilities: {caps:#?}");

    let sig_ctx = transport.get_signature_context().await?;
    println!("global_id: {}", sig_ctx.global_id);
    println!("capabilities: {:?}", sig_ctx.capabilities);

    println!();
    println!("==============================");
    println!("Account test module");
    println!("==============================");

    let address =
        StdAddr::from_str("0:497005b6d06782de9ca7e33339c278be33df5c09531988eca67e8926a975b93d")?;

    println!("Address: {address}");

    let contract_state = transport.get_contract_state(&address).await?;
    println!("Contract state: {contract_state:#?}");

    let account_status = transport.get_account_status(&address).await?;
    println!("Account status: {account_status:?}");

    let blockchain_config = transport.get_blockchain_config().await?;
    println!(
        "Blockchain config global_id: {}",
        blockchain_config.global_id
    );
    println!("Blockchain config seqno: {}", blockchain_config.seqno);
    println!("Blockchain config: {:?}", blockchain_config);

    let mut wallet = EverWallet::new(keys_0, transport)?;

    println!("Wallet address: {}", wallet.address());
    println!("Wallet status: {:?}", wallet.status().await?);
    println!("Wallet balance: {:?}", wallet.balance().await?);

    let status = wallet.refresh().await?;
    println!("Refreshed status: {:?}", status);

    Ok(())
}
