use std::borrow::Cow;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use tycho_types::models::{
    Account, AccountStatus, BlockchainConfig, GlobalCapabilities, GlobalCapability,
    SignatureDomain, StdAddr,
};
use tycho_types::prelude::{Boc, BocRepr, Cell, DynCell, Load};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureContext {
    pub global_id: i32,
    pub capabilities: GlobalCapabilities,
}

impl SignatureContext {
    pub fn uses_signature_id(&self) -> bool {
        self.capabilities
            .contains(GlobalCapability::CapSignatureWithId)
    }

    pub fn uses_signature_domain(&self) -> bool {
        self.capabilities
            .contains(GlobalCapability::CapSignatureDomain)
    }

    pub fn signature_domain(&self) -> SignatureDomain {
        if self.uses_signature_domain() {
            SignatureDomain::L2 {
                global_id: self.global_id,
            }
        } else {
            SignatureDomain::Empty
        }
    }

    pub fn legacy_signature_id(&self) -> Option<i32> {
        if self.uses_signature_id() && !self.uses_signature_domain() {
            Some(self.global_id)
        } else {
            None
        }
    }

    pub fn apply<'a>(&self, data: &'a [u8]) -> Cow<'a, [u8]> {
        if !self.uses_signature_id() {
            return Cow::Borrowed(data);
        }

        if self.uses_signature_domain() {
            self.signature_domain().apply(data)
        } else {
            let mut result = Vec::with_capacity(4 + data.len());
            result.extend_from_slice(&self.global_id.to_be_bytes());
            result.extend_from_slice(data);
            Cow::Owned(result)
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum GetContractStateResponse {
    NotExists {},
    Exists {
        #[serde(deserialize_with = "deserialize_account")]
        account: Box<Account>,
    },
    Unchanged {},
}

fn deserialize_account<'de, D>(deserializer: D) -> Result<Box<Account>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    fn read_account(cell: Cell) -> std::result::Result<Box<Account>, tycho_types::error::Error> {
        let s = &mut cell.as_slice()?;

        Ok(Box::new(Account {
            address: <_>::load_from(s)?,
            storage_stat: <_>::load_from(s)?,
            last_trans_lt: <_>::load_from(s)?,
            balance: <_>::load_from(s)?,
            state: <_>::load_from(s)?,
        }))
    }

    Boc::deserialize(deserializer).and_then(|cell| read_account(cell).map_err(Error::custom))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBlockchainConfigResponse {
    pub global_id: i32,
    pub seqno: u32,
    #[serde(with = "BocRepr")]
    pub config: BlockchainConfig,
}

#[derive(Debug, Deserialize)]
struct JrpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct JrpcResponse<T> {
    result: Option<T>,
    error: Option<JrpcError>,
}

#[derive(Clone)]
pub struct JrpcTransport {
    client: reqwest::Client,
    base_url: Url,
}

impl JrpcTransport {
    pub fn new(url: &str) -> Result<Self> {
        Self::with_timeout(url, Duration::from_secs(15))
    }

    pub fn with_timeout(url: &str, timeout: Duration) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        let client = reqwest::ClientBuilder::new()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(5))
            .timeout(timeout)
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self {
            client,
            base_url: Url::parse(url).context("invalid JRPC URL")?,
        })
    }

    pub async fn get_capabilities(&self) -> Result<Vec<String>> {
        self.call("getCapabilities", &()).await
    }

    pub async fn get_contract_state(&self, address: &StdAddr) -> Result<GetContractStateResponse> {
        #[derive(Serialize)]
        struct Params<'a> {
            address: &'a StdAddr,
        }

        self.call("getContractState", &Params { address }).await
    }

    pub async fn get_account_status(&self, address: &StdAddr) -> Result<AccountStatus> {
        match self.get_contract_state(address).await? {
            GetContractStateResponse::NotExists {} => Ok(AccountStatus::NotExists),
            GetContractStateResponse::Exists { account } => Ok(account.state.status()),
            GetContractStateResponse::Unchanged {} => {
                bail!("unexpected getContractState response: Unchanged")
            }
        }
    }

    pub async fn get_blockchain_config(&self) -> Result<GetBlockchainConfigResponse> {
        self.call("getBlockchainConfig", &()).await
    }

    pub async fn get_signature_context(&self) -> Result<SignatureContext> {
        let response = self.get_blockchain_config().await?;
        let capabilities = response.config.get_global_version()?.capabilities;

        Ok(SignatureContext {
            global_id: response.global_id,
            capabilities,
        })
    }

    pub async fn send_message(&self, message: &DynCell) -> Result<()> {
        #[derive(Serialize)]
        struct Params<'a> {
            message: &'a str,
        }

        let message = Boc::encode_base64(message);

        self.call_unit("sendMessage", &Params { message: &message })
            .await
    }

    async fn call<P, R>(&self, method: &str, params: &P) -> Result<R>
    where
        P: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let text = self.post_raw(method, params).await?;

        let response: JrpcResponse<R> = serde_json::from_str(&text)
            .with_context(|| format!("invalid JRPC response for method {method}: {text}"))?;

        match (response.result, response.error) {
            (Some(result), None) => Ok(result),
            (None, Some(err)) => bail!("RPC error {}: {}", err.code, err.message),
            (None, None) => bail!("RPC returned null result for method {method}: {text}"),
            (Some(_), Some(err)) => bail!(
                "RPC returned both result and error for method {method}: {} {}",
                err.code,
                err.message
            ),
        }
    }

    async fn call_unit<P>(&self, method: &str, params: &P) -> Result<()>
    where
        P: Serialize + ?Sized,
    {
        let text = self.post_raw(method, params).await?;

        let response: JrpcResponse<serde_json::Value> = serde_json::from_str(&text)
            .with_context(|| format!("invalid JRPC response for method {method}: {text}"))?;

        match (response.result, response.error) {
            (Some(_), None) => Ok(()),
            (None, None) => Ok(()),
            (None, Some(err)) => bail!("RPC error {}: {}", err.code, err.message),
            (Some(_), Some(err)) => bail!(
                "RPC returned both result and error for method {method}: {} {}",
                err.code,
                err.message
            ),
        }
    }

    async fn post_raw<P>(&self, method: &str, params: &P) -> Result<String>
    where
        P: Serialize + ?Sized,
    {
        #[derive(Serialize)]
        struct Request<'a, T: ?Sized> {
            jsonrpc: &'static str,
            id: u32,
            method: &'a str,
            params: &'a T,
        }

        let body = Request {
            jsonrpc: "2.0",
            id: 1,
            method,
            params,
        };

        let response = self
            .client
            .post(self.base_url.clone())
            .json(&body)
            .send()
            .await
            .context("failed to send HTTP request")?
            .error_for_status()
            .context("HTTP request failed")?;

        response
            .text()
            .await
            .context("failed to read HTTP response body")
    }
}
