use anyhow::{Result, bail};
use tycho_types::models::{Account, AccountState, AccountStatus};
use tycho_types::num::{Tokens, VarUint56};
use tycho_types::prelude::{Cell, HashBytes};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountStateInfo {
    Uninit,
    Frozen(HashBytes),
    Active,
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub address: String,
    pub status: AccountStatus,
    pub state: AccountStateInfo,

    pub last_trans_lt: u64,

    pub used_cells: VarUint56,
    pub used_bits: VarUint56,
    pub last_paid: u32,

    pub balance_tokens: Tokens,

    pub code: Option<Cell>,
    pub data: Option<Cell>,
    pub code_hash: Option<HashBytes>,
    pub data_hash: Option<HashBytes>,
}

impl AccountInfo {
    pub fn from_account(account: &Account) -> Self {
        let (state, code, data, code_hash, data_hash) = match &account.state {
            AccountState::Uninit => (AccountStateInfo::Uninit, None, None, None, None),
            AccountState::Frozen(hash) => (AccountStateInfo::Frozen(*hash), None, None, None, None),
            AccountState::Active(state_init) => {
                let code = state_init.code.clone();
                let data = state_init.data.clone();

                let code_hash = code.as_ref().map(|cell| *cell.repr_hash());
                let data_hash = data.as_ref().map(|cell| *cell.repr_hash());

                (AccountStateInfo::Active, code, data, code_hash, data_hash)
            }
        };

        Self {
            address: account.address.to_string(),
            status: account.state.status(),
            state,

            last_trans_lt: account.last_trans_lt,

            used_cells: account.storage_stat.used.cells,
            used_bits: account.storage_stat.used.bits,
            last_paid: account.storage_stat.last_paid,

            balance_tokens: account.balance.tokens,

            code,
            data,
            code_hash,
            data_hash,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, AccountStateInfo::Active)
    }

    pub fn is_uninit(&self) -> bool {
        matches!(self.state, AccountStateInfo::Uninit)
    }

    pub fn is_frozen(&self) -> bool {
        matches!(self.state, AccountStateInfo::Frozen(_))
    }
}

#[derive(Debug, Clone)]
pub enum ContractState {
    NotExists,
    Unchanged,
    Exists(Box<AccountInfo>),
}

impl ContractState {
    pub fn exists(&self) -> bool {
        matches!(self, Self::Exists(_))
    }

    pub fn as_account(&self) -> Option<&AccountInfo> {
        match self {
            Self::Exists(info) => Some(info.as_ref()),
            _ => None,
        }
    }

    pub fn into_account(self) -> Option<AccountInfo> {
        match self {
            Self::Exists(info) => Some(*info),
            _ => None,
        }
    }

    pub fn expect_account(self) -> Result<AccountInfo> {
        match self {
            Self::Exists(info) => Ok(*info),
            Self::NotExists => bail!("account does not exist"),
            Self::Unchanged => bail!("account state unchanged"),
        }
    }
}
