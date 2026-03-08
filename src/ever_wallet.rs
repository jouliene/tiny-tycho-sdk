use base64::Engine;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use ed25519_dalek::VerifyingKey;
use tycho_types::abi::{
    AbiHeaderType, AbiVersion, Function, IntoAbi, UnsignedExternalMessage, WithAbiType,
    extend_signature_with_id,
};
use tycho_types::models::{AccountStatus, OwnedMessage, StateInit, StdAddr};
use tycho_types::num::Tokens;
use tycho_types::prelude::*;

use crate::Keys;
use crate::transport::jrpc::{GetContractStateResponse, JrpcTransport, SignatureContext};

pub const MSG_FLAGS_SEPARATE_FEES: u8 = 1;
pub const MSG_FLAGS_IGNORE_ACTION_ERRORS: u8 = 2;
pub const MSG_FLAGS_SIMPLE_SEND: u8 = 3;
pub const MSG_FLAGS_DESTROY_IF_ZERO: u8 = 32;
pub const MSG_FLAGS_CARRY_REMAINING_INBOUND_VALUE: u8 = 64;
pub const MSG_FLAGS_SEND_ALL_BALANCE: u8 = 128;

const DEFAULT_TTL_SECS: u32 = 60;

/// Published EVER Wallet contract code from broxus/ever-wallet-contract README.
const EVER_WALLET_CODE_BOC_BASE64: &str = "te6cckEBBgEA/AABFP8A9KQT9LzyyAsBAgEgAgMABNIwAubycdcBAcAA8nqDCNcY7UTQgwfXAdcLP8j4KM8WI88WyfkAA3HXAQHDAJqDB9cBURO68uBk3oBA1wGAINcBgCDXAVQWdfkQ8qj4I7vyeWa++COBBwiggQPoqFIgvLHydAIgghBM7mRsuuMPAcjL/8s/ye1UBAUAmDAC10zQ+kCDBtcBcdcBeNcB10z4AHCAEASqAhSxyMsFUAXPFlAD+gLLaSLQIc8xIddJoIQJuZgzcAHLAFjPFpcwcQHLABLM4skB+wAAPoIQFp4+EbqOEfgAApMg10qXeNcB1AL7AOjRkzLyPOI+zYS/";

#[derive(Debug, Clone, WithAbiType, IntoAbi)]
struct SendTransactionInputs {
    dest: StdAddr,
    value: u128,
    bounce: bool,
    flags: u8,
    payload: Cell,
}

#[derive(Debug, Clone)]
pub struct SendReceipt {
    pub message_hash: String,
    pub sent_at_ms: u64,
}

impl std::fmt::Display for SendReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} @ {}", self.message_hash, self.sent_at_ms)
    }
}

pub struct EverWallet {
    transport: JrpcTransport,
    keys: Keys,
    address: StdAddr,
    workchain: i8,
    cached_status: Option<AccountStatus>,
    signature_context: Option<SignatureContext>,
    last_used_ms: u64,
}

impl EverWallet {
    pub fn new(keys: Keys, transport: JrpcTransport) -> Result<Self> {
        Self::new_in_workchain(keys, transport, 0)
    }

    pub fn new_in_workchain(keys: Keys, transport: JrpcTransport, workchain: i8) -> Result<Self> {
        let address = Self::compute_address(workchain, keys.public_key())?;

        Ok(Self {
            transport,
            keys,
            address,
            workchain,
            cached_status: None,
            signature_context: None,
            last_used_ms: 0,
        })
    }

    pub fn address(&self) -> &StdAddr {
        &self.address
    }

    pub fn workchain(&self) -> i8 {
        self.workchain
    }

    pub fn public_key(&self) -> &VerifyingKey {
        self.keys.public_key()
    }

    pub fn public_key_hex(&self) -> String {
        self.keys.public_key_hex()
    }

    pub fn state_init(&self) -> Result<StateInit> {
        make_state_init(self.keys.public_key())
    }

    pub fn state_init_cell(&self) -> Result<Cell> {
        CellBuilder::build_from(self.state_init()?)
            .context("failed to build wallet state init cell")
    }

    pub fn compute_address(workchain: i8, public_key: &VerifyingKey) -> Result<StdAddr> {
        compute_wallet_address(workchain, public_key)
    }

    pub async fn status(&self) -> Result<AccountStatus> {
        self.transport.get_account_status(&self.address).await
    }

    pub async fn balance(&self) -> Result<Option<Tokens>> {
        match self.transport.get_contract_state(&self.address).await? {
            GetContractStateResponse::NotExists {} => Ok(None),
            GetContractStateResponse::Exists { account } => Ok(Some(account.balance.tokens)),
            GetContractStateResponse::Unchanged {} => {
                bail!("unexpected getContractState response: Unchanged")
            }
        }
    }

    pub async fn refresh(&mut self) -> Result<AccountStatus> {
        let status = self.transport.get_account_status(&self.address).await?;
        let signature_context = self.transport.get_signature_context().await?;

        self.cached_status = Some(status);
        self.signature_context = Some(signature_context);

        Ok(status)
    }

    pub async fn send(&mut self, dest: &StdAddr, value: u128) -> Result<SendReceipt> {
        self.send_transaction(dest, value, MSG_FLAGS_SIMPLE_SEND, false, None)
            .await
    }

    pub async fn send_transaction(
        &mut self,
        dest: &StdAddr,
        value: u128,
        flags: u8,
        bounce: bool,
        payload: Option<&Cell>,
    ) -> Result<SendReceipt> {
        let _ = self.prepare().await?;

        let (message_cell, message_hash, sent_at_ms, deployed_now) =
            self.build_send_transaction_cell(dest, value, flags, bounce, payload)?;

        self.transport.send_message(message_cell.as_ref()).await?;

        if deployed_now {
            self.cached_status = Some(AccountStatus::Active);
        }

        Ok(SendReceipt {
            message_hash,
            sent_at_ms,
        })
    }

    async fn prepare(&mut self) -> Result<(AccountStatus, SignatureContext)> {
        if self.cached_status.is_none() {
            self.cached_status = Some(self.transport.get_account_status(&self.address).await?);
        }

        if self.signature_context.is_none() {
            self.signature_context = Some(self.transport.get_signature_context().await?);
        }

        let status = self
            .cached_status
            .ok_or_else(|| anyhow!("wallet status is not initialized"))?;

        let signature_context = self
            .signature_context
            .ok_or_else(|| anyhow!("wallet signature context is not initialized"))?;

        Ok((status, signature_context))
    }

    fn build_send_transaction_cell(
        &mut self,
        dest: &StdAddr,
        value: u128,
        flags: u8,
        bounce: bool,
        payload: Option<&Cell>,
    ) -> Result<(Cell, String, u64, bool)> {
        let (status, signature_context) = self.require_prepared()?;

        let now_ms = next_unique_time_ms(&mut self.last_used_ms)?;
        let expire_at = (now_ms / 1000) as u32 + DEFAULT_TTL_SECS;
        let payload = payload.unwrap_or(empty_payload_cell()?);

        let inputs = SendTransactionInputs {
            dest: dest.clone(),
            value,
            bounce,
            flags,
            payload: payload.clone(),
        };

        let unsigned = send_transaction_fn()
            .encode_external(&inputs.into_abi().into_tuple().unwrap())
            .with_time(now_ms)
            .with_expire_at(expire_at)
            .with_pubkey(self.keys.public_key())
            .build_message(&self.address)?;

        self.finalize_unsigned_message(unsigned, status, signature_context, now_ms)
    }

    fn require_prepared(&self) -> Result<(AccountStatus, SignatureContext)> {
        let status = self
            .cached_status
            .ok_or_else(|| anyhow!("wallet is not prepared; call refresh().await? first"))?;

        let signature_context = self
            .signature_context
            .ok_or_else(|| anyhow!("wallet is not prepared; call refresh().await? first"))?;

        Ok((status, signature_context))
    }

    fn finalize_unsigned_message(
        &self,
        unsigned: UnsignedExternalMessage,
        status: AccountStatus,
        signature_context: SignatureContext,
        sent_at_ms: u64,
    ) -> Result<(Cell, String, u64, bool)> {
        let (unsigned, deployed_now) = match status {
            AccountStatus::Active => (unsigned, false),
            AccountStatus::Uninit | AccountStatus::NotExists => {
                (unsigned.with_state_init(self.state_init()?), true)
            }
            AccountStatus::Frozen => bail!("sender account is frozen"),
        };

        let message = self.sign_unsigned_message(unsigned, signature_context)?;
        let message_cell =
            CellBuilder::build_from(message).context("failed to build signed external message")?;
        let message_hash = format!("{}", *message_cell.repr_hash());

        Ok((message_cell, message_hash, sent_at_ms, deployed_now))
    }

    fn sign_unsigned_message(
        &self,
        unsigned: UnsignedExternalMessage,
        signature_context: SignatureContext,
    ) -> Result<OwnedMessage> {
        let signature = if signature_context.uses_signature_domain() {
            let data = signature_context
                .domain
                .apply(unsigned.body.hash.as_slice());
            self.keys.sign(data.as_ref())
        } else {
            let data = extend_signature_with_id(
                unsigned.body.hash.as_slice(),
                signature_context.legacy_signature_id(),
            );
            self.keys.sign(data.as_ref())
        };

        unsigned
            .with_signature(&signature)
            .context("failed to attach signature to external message")
    }
}

impl std::fmt::Display for EverWallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.address)
    }
}

impl std::fmt::Debug for EverWallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EverWallet")
            .field("address", &self.address)
            .field("workchain", &self.workchain)
            .field("public_key", &self.public_key_hex())
            .field("cached_status", &self.cached_status)
            .field("prepared", &self.signature_context.is_some())
            .finish()
    }
}

fn wallet_code_cell() -> Result<&'static Cell> {
    static CODE: OnceLock<std::result::Result<Cell, String>> = OnceLock::new();

    match CODE.get_or_init(|| {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(EVER_WALLET_CODE_BOC_BASE64)
            .map_err(|e| e.to_string())?;

        Boc::decode(bytes.as_slice()).map_err(|e| e.to_string())
    }) {
        Ok(cell) => Ok(cell),
        Err(msg) => bail!("failed to load embedded EVER wallet code: {msg}"),
    }
}

fn empty_payload_cell() -> Result<&'static Cell> {
    static EMPTY: OnceLock<std::result::Result<Cell, String>> = OnceLock::new();

    match EMPTY.get_or_init(|| {
        let cx = Cell::empty_context();
        CellBuilder::new().build_ext(cx).map_err(|e| e.to_string())
    }) {
        Ok(cell) => Ok(cell),
        Err(msg) => bail!("failed to build empty payload cell: {msg}"),
    }
}

fn make_state_init(public: &VerifyingKey) -> Result<StateInit> {
    let data = CellBuilder::build_from((HashBytes(public.to_bytes()), 0u64))
        .context("failed to build wallet state data")?;

    Ok(StateInit {
        split_depth: None,
        special: None,
        code: Some(wallet_code_cell()?.clone()),
        data: Some(data),
        libraries: Dict::new(),
    })
}

fn compute_wallet_address(workchain: i8, public: &VerifyingKey) -> Result<StdAddr> {
    let hash = *CellBuilder::build_from(make_state_init(public)?)
        .context("failed to build state init cell")?
        .repr_hash();

    Ok(StdAddr::new(workchain, hash))
}

fn send_transaction_fn() -> &'static Function {
    static ABI: OnceLock<Function> = OnceLock::new();

    ABI.get_or_init(|| {
        Function::builder(AbiVersion::V2_3, "sendTransaction")
            .with_headers([
                AbiHeaderType::PublicKey,
                AbiHeaderType::Time,
                AbiHeaderType::Expire,
            ])
            .with_inputs(SendTransactionInputs::abi_type().named("").flatten())
            .build()
    })
}

fn next_unique_time_ms(last_used_ms: &mut u64) -> Result<u64> {
    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;

    let next = if now_ms <= *last_used_ms {
        *last_used_ms + 1
    } else {
        now_ms
    };

    *last_used_ms = next;
    Ok(next)
}

trait IntoTuple {
    fn into_tuple(self) -> Option<Vec<tycho_types::abi::NamedAbiValue>>;
}

impl IntoTuple for tycho_types::abi::AbiValue {
    fn into_tuple(self) -> Option<Vec<tycho_types::abi::NamedAbiValue>> {
        match self {
            tycho_types::abi::AbiValue::Tuple(values) => Some(values),
            _ => None,
        }
    }
}
