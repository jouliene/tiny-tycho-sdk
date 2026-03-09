use anyhow::Result;
use std::str::FromStr;
use tiny_tycho_sdk::{
    EverWallet, JrpcTransport, Keys, SEND_FLAG_BOUNCE_IF_ACTION_FAIL, SEND_FLAG_DESTROY_IF_ZERO,
    SEND_FLAG_IGNORE_ERRORS, SEND_FLAG_PAY_FWD_FEES_SEPARATELY, SEND_MODE_ALL_BALANCE_AND_DESTROY,
    SEND_MODE_CARRY_REMAINING_INBOUND_VALUE, SEND_MODE_ORDINARY, SEND_MODE_SEND_ALL_BALANCE,
    SEND_MODE_SIMPLE_SEND, SendReceipt, StdAddr,
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("==================================================");
    println!("tiny-tycho-sdk full example");
    println!("==================================================");

    // -------------------------------------------------
    // CONFIG
    // -------------------------------------------------
    // Pick one endpoint:
    let rpc_url = "https://rpc-testnet.tychoprotocol.com";
    // let rpc_url = "https://jrpc.everwallet.net";

    // Known phrase used across examples
    let phrase = "alter sustain pulp catalog announce tail bunker mammal figure burger party title";

    // Example destination
    let dest =
        StdAddr::from_str("0:bb6b6df4771627fe2612e74ed01168e1abb9e035b09f5e787ef30667303e61c2")?;

    // Toggle these manually if you want to actually broadcast
    let do_simple_send = true;
    let do_full_send = true;

    // -------------------------------------------------
    // 1. KEYS MODULE EXAMPLES
    // -------------------------------------------------
    println!();
    println!("==================================================");
    println!("1. KEYS MODULE");
    println!("==================================================");

    // derive from seed phrase (default index = 0)
    let keys_0 = Keys::from_seed_phrase(phrase)?;
    println!("keys_0 public hex: {}", keys_0.public_key_hex());
    println!("keys_0 secret hex: {}", keys_0.secret_key_hex());
    println!("keys_0 public bytes: {:02x?}", keys_0.public_key_bytes());
    println!("keys_0 secret bytes: {:02x?}", keys_0.secret_key_bytes());

    // derive from seed phrase with explicit index
    let keys_1 = Keys::from_seed_phrase_with_index(phrase, 1)?;
    println!("keys_1 public hex: {}", keys_1.public_key_hex());
    println!("keys_1 secret hex: {}", keys_1.secret_key_hex());

    let keys_2 = Keys::from_seed_phrase_with_index(phrase, 2)?;
    println!("keys_2 public hex: {}", keys_2.public_key_hex());
    println!("keys_2 secret hex: {}", keys_2.secret_key_hex());

    // generate random seed phrase
    let generated_phrase = Keys::generate_seed_phrase()?;
    println!("generated seed phrase: {}", generated_phrase);

    // derive keys from generated seed phrase
    let generated_phrase_keys = Keys::from_seed_phrase(&generated_phrase)?;
    println!(
        "generated phrase public hex: {}",
        generated_phrase_keys.public_key_hex()
    );

    // generate random keypair directly
    let random_keys = Keys::generate_keys();
    println!("random public hex: {}", random_keys.public_key_hex());
    println!("random secret hex: {}", random_keys.secret_key_hex());

    // derive from secret hex
    let derived_from_hex = Keys::from_secret_hex_str(
        "f97ca5343717b0ea2f2234562ceb0e5cc53b7eb7a2519385049a736f78e51432",
    )?;
    println!(
        "derived-from-hex public hex: {}",
        derived_from_hex.public_key_hex()
    );
    println!(
        "derived-from-hex secret hex: {}",
        derived_from_hex.secret_key_hex()
    );

    // sign + verify example
    let message = b"hello from tiny-tycho-sdk";
    let signature = keys_0.sign(message);
    println!("signature bytes: {:02x?}", signature.to_bytes());
    println!("verify with keys_0: {}", keys_0.verify(message, &signature));
    println!("verify with keys_1: {}", keys_1.verify(message, &signature));

    // public_key() returns &VerifyingKey
    println!("keys_0 verifying key debug: {:?}", keys_0.public_key());

    // -------------------------------------------------
    // 2. JRPC TRANSPORT EXAMPLES
    // -------------------------------------------------
    println!();
    println!("==================================================");
    println!("2. JRPC TRANSPORT");
    println!("==================================================");

    let transport = JrpcTransport::new(rpc_url)?;
    println!("rpc url: {rpc_url}");

    // get capabilities
    let capabilities = transport.get_capabilities().await?;
    println!("capabilities: {capabilities:#?}");

    // get signature context
    let sig_ctx = transport.get_signature_context().await?;
    println!("signature context: {:?}", sig_ctx);
    println!("uses signature_id: {}", sig_ctx.uses_signature_id());
    println!("uses signature_domain: {}", sig_ctx.uses_signature_domain());
    println!("signature domain: {:?}", sig_ctx.signature_domain());
    println!("legacy signature id: {:?}", sig_ctx.legacy_signature_id());

    // show how signing preimage is transformed by context
    let applied = sig_ctx.apply(message);
    println!("original message bytes: {:02x?}", message);
    println!("applied signing bytes: {:02x?}", applied);

    // blockchain config
    let blockchain_config = transport.get_blockchain_config().await?;
    println!("blockchain global_id: {}", blockchain_config.global_id);
    println!("blockchain seqno: {}", blockchain_config.seqno);
    println!("blockchain config debug: {:?}", blockchain_config);

    // contract/account state
    println!("destination address: {dest}");

    let contract_state = transport.get_contract_state(&dest).await?;
    println!("contract state: {contract_state:#?}");

    let account_status = transport.get_account_status(&dest).await?;
    println!("account status: {account_status:?}");

    // -------------------------------------------------
    // 3. EVER WALLET EXAMPLES
    // -------------------------------------------------
    println!();
    println!("==================================================");
    println!("3. EVER WALLET");
    println!("==================================================");

    // wallet from keys_0
    let mut wallet = EverWallet::new(Keys::from_seed_phrase(phrase)?, transport.clone())?;

    println!("wallet debug: {:?}", wallet);
    println!("wallet display/address: {}", wallet);
    println!("wallet address(): {}", wallet.address());
    println!("wallet workchain(): {}", wallet.workchain());
    println!("wallet public_key(): {:?}", wallet.public_key());
    println!("wallet public_key_hex(): {}", wallet.public_key_hex());

    // compute address from public key explicitly
    let recomputed = EverWallet::compute_address(wallet.workchain(), wallet.public_key())?;
    println!("recomputed address: {recomputed}");
    println!("address matches: {}", &recomputed == wallet.address());

    // construct same wallet in explicit workchain
    let wallet_wc0 =
        EverWallet::new_in_workchain(Keys::from_seed_phrase(phrase)?, transport.clone(), 0)?;
    println!("wallet_wc0 address: {}", wallet_wc0.address());

    // state init
    let state_init = wallet.state_init()?;
    println!("wallet state_init: {state_init:#?}");

    let state_init_cell = wallet.state_init_cell()?;
    println!(
        "wallet state_init cell hash: {}",
        state_init_cell.repr_hash()
    );

    // live wallet state
    println!("wallet status(): {:?}", wallet.status().await?);
    println!("wallet balance(): {:?}", wallet.balance().await?);

    let refreshed_status = wallet.refresh().await?;
    println!("wallet refresh(): {:?}", refreshed_status);
    println!("wallet after refresh debug: {:?}", wallet);

    let info = wallet
        .account_info()
        .await?
        .expect("wallet account must exists");

    println!("wallet address: {}", info.address);
    println!("wallet status: {:?}", info.status);
    println!("wallet balance: {:?}", info.balance_tokens);
    println!("wallet cells: {:?}", info.used_cells);
    println!("wallet bits: {:?}", info.used_bits);
    println!("wallet code hash: {:?}", info.code_hash);
    println!("wallet data hash: {:?}", info.data_hash);

    // -------------------------------------------------
    // 4. SEND MODES / FLAGS
    // -------------------------------------------------
    println!();
    println!("==================================================");
    println!("4. SEND MODES / FLAGS");
    println!("==================================================");

    println!("SEND_MODE_ORDINARY: {}", SEND_MODE_ORDINARY);
    println!(
        "SEND_MODE_CARRY_REMAINING_INBOUND_VALUE: {}",
        SEND_MODE_CARRY_REMAINING_INBOUND_VALUE
    );
    println!("SEND_MODE_SEND_ALL_BALANCE: {}", SEND_MODE_SEND_ALL_BALANCE);
    println!(
        "SEND_FLAG_PAY_FWD_FEES_SEPARATELY: {}",
        SEND_FLAG_PAY_FWD_FEES_SEPARATELY
    );
    println!("SEND_FLAG_IGNORE_ERRORS: {}", SEND_FLAG_IGNORE_ERRORS);
    println!(
        "SEND_FLAG_BOUNCE_IF_ACTION_FAIL: {}",
        SEND_FLAG_BOUNCE_IF_ACTION_FAIL
    );
    println!("SEND_FLAG_DESTROY_IF_ZERO: {}", SEND_FLAG_DESTROY_IF_ZERO);
    println!("SEND_MODE_SIMPLE_SEND: {}", SEND_MODE_SIMPLE_SEND);
    println!(
        "SEND_MODE_ALL_BALANCE_AND_DESTROY: {}",
        SEND_MODE_ALL_BALANCE_AND_DESTROY
    );

    let custom_mode =
        SEND_MODE_ORDINARY | SEND_FLAG_PAY_FWD_FEES_SEPARATELY | SEND_FLAG_IGNORE_ERRORS;
    println!("custom_mode example: {}", custom_mode);

    // -------------------------------------------------
    // 5. SIMPLE SEND EXAMPLE
    // -------------------------------------------------
    println!();
    println!("==================================================");
    println!("5. SIMPLE SEND");
    println!("==================================================");

    if do_simple_send {
        let receipt: SendReceipt = wallet.send(&dest, 1_000_000).await?;
        println!("simple send accepted by JRPC");
        println!("receipt: {}", receipt);
        println!("message_hash: {}", receipt.message_hash);
        println!("sent_at_ms: {}", receipt.sent_at_ms);
    } else {
        println!("simple send skipped (do_simple_send = false)");
        println!("example call:");
        println!(r#"let receipt = wallet.send(&dest, 1_000_000).await?;"#);
    }

    // -------------------------------------------------
    // 6. FULL send_transaction EXAMPLE
    // -------------------------------------------------
    println!();
    println!("==================================================");
    println!("6. FULL send_transaction");
    println!("==================================================");

    let full_mode =
        SEND_MODE_ORDINARY | SEND_FLAG_PAY_FWD_FEES_SEPARATELY | SEND_FLAG_IGNORE_ERRORS;

    let bounce = false;
    let payload = None;

    if do_full_send {
        let receipt = wallet
            .send_transaction(&dest, 5_000_000, full_mode, bounce, payload)
            .await?;

        println!("full send_transaction accepted by JRPC");
        println!("receipt: {}", receipt);
        println!("message_hash: {}", receipt.message_hash);
        println!("sent_at_ms: {}", receipt.sent_at_ms);
    } else {
        println!("full send skipped (do_full_send = false)");
        println!("example call:");
        println!(
            r#"let receipt = wallet.send_transaction(&dest, 5_000_000, full_mode, false, None).await?;"#
        );
    }

    // -------------------------------------------------
    // 7. EXTRA SEQUENTIAL SEND EXAMPLE
    // -------------------------------------------------
    println!();
    println!("==================================================");
    println!("7. SEQUENTIAL SEND EXAMPLE");
    println!("==================================================");

    println!("Example loop for multiple sequential sends:");
    println!(
        r#"
for i in 0..10 {{
    let receipt = wallet.send(&dest, 1_000_000).await?;
    println!("tx #{{}} => {{}}", i + 1, receipt);
}}
"#
    );

    println!();
    println!("Done.");
    Ok(())
}
