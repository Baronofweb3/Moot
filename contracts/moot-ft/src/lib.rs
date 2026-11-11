//! MOOT fixed-supply token (NEP-141 + NEP-145 + NEP-148).
//!
//! The whole supply is minted once, inside `new`, to `holder_id` (the MOOT core contract,
//! which puts it straight into the trading pool). There is no mint function, no owner and no
//! privileged account. Burning is not exposed either, so total supply can never change.

use near_contract_standards::fungible_token::events::FtMint;
use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider,
};
use near_contract_standards::fungible_token::{FungibleToken, FungibleTokenCore, FungibleTokenResolver};
use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::borsh::BorshSerialize;
use near_sdk::collections::LazyOption;
use near_sdk::json_types::U128;
use near_sdk::{near, require, AccountId, BorshStorageKey, NearToken, PanicOnDefault, PromiseOrValue};

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    Token,
    Metadata,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    token: FungibleToken,
    metadata: LazyOption<FungibleTokenMetadata>,
}

#[near]
impl Contract {
    /// One-time initialisation. Called in the same batch that deploys the code, so nobody can
    /// front-run it. Mints the entire fixed supply to `holder_id` and nothing else, ever.
    #[init]
    pub fn new(holder_id: AccountId, total_supply: U128, metadata: FungibleTokenMetadata) -> Self {
        require!(total_supply.0 > 0, "supply must be positive");
        metadata.assert_valid();
        let mut this = Self {
            token: FungibleToken::new(StorageKey::Token),
            metadata: LazyOption::new(StorageKey::Metadata, Some(&metadata)),
        };
        this.token.internal_register_account(&holder_id);
        this.token.internal_deposit(&holder_id, total_supply.0);
        FtMint {
            owner_id: &holder_id,
            amount: total_supply,
            memo: Some("fixed supply minted once at creation"),
        }
        .emit();
        this
    }
}

#[near]
impl FungibleTokenCore for Contract {
    #[payable]
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>) {
        self.token.ft_transfer(receiver_id, amount, memo)
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.token.ft_transfer_call(receiver_id, amount, memo, msg)
    }

    fn ft_total_supply(&self) -> U128 {
        self.token.ft_total_supply()
    }

    fn ft_balance_of(&self, account_id: AccountId) -> U128 {
        self.token.ft_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenResolver for Contract {
    #[private]
    fn ft_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: U128,
    ) -> U128 {
        // Burning is never allowed, so a resolve can never reduce total supply.
        let (used, burned) = self.token.internal_ft_resolve_transfer(&sender_id, receiver_id, amount);
        require!(burned == 0, "burn not supported");
        used.into()
    }
}

#[near]
impl StorageManagement for Contract {
    #[payable]
    fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        self.token.storage_deposit(account_id, registration_only)
    }

    #[payable]
    fn storage_withdraw(&mut self, amount: Option<NearToken>) -> StorageBalance {
        self.token.storage_withdraw(amount)
    }

    #[payable]
    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        // force=true would burn the caller's tokens and shrink total supply: not allowed.
        require!(force != Some(true), "force unregister (burning) is not supported");
        self.token.storage_unregister(force)
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        self.token.storage_balance_bounds()
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.token.storage_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        self.metadata.get().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_contract_standards::fungible_token::metadata::FT_METADATA_SPEC;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, NearToken};

    fn meta() -> FungibleTokenMetadata {
        FungibleTokenMetadata {
            spec: FT_METADATA_SPEC.to_string(),
            name: "Test".into(),
            symbol: "TST".into(),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 18,
        }
    }
    fn ctx(pred: AccountId, deposit: u128) -> VMContextBuilder {
        let mut b = VMContextBuilder::new();
        b.predecessor_account_id(pred.clone())
            .signer_account_id(pred)
            .attached_deposit(NearToken::from_yoctonear(deposit));
        b
    }

    #[test]
    fn whole_supply_goes_to_holder_and_total_is_fixed() {
        testing_env!(ctx(accounts(0), 0).build());
        let c = Contract::new(accounts(1), U128(1_000_000), meta());
        assert_eq!(c.ft_total_supply().0, 1_000_000);
        assert_eq!(c.ft_balance_of(accounts(1)).0, 1_000_000);
        assert_eq!(c.ft_balance_of(accounts(0)).0, 0);
    }

    #[test]
    fn transfers_never_change_total_supply() {
        testing_env!(ctx(accounts(0), 0).build());
        let mut c = Contract::new(accounts(1), U128(1_000), meta());
        // register receiver
        testing_env!(ctx(accounts(2), 2_000_000_000_000_000_000_000).build());
        c.storage_deposit(Some(accounts(2)), Some(true));
        testing_env!(ctx(accounts(1), 1).build());
        c.ft_transfer(accounts(2), U128(400), None);
        assert_eq!(c.ft_balance_of(accounts(1)).0, 600);
        assert_eq!(c.ft_balance_of(accounts(2)).0, 400);
        assert_eq!(c.ft_total_supply().0, 1_000);
    }

    #[test]
    #[should_panic(expected = "supply must be positive")]
    fn zero_supply_rejected() {
        testing_env!(ctx(accounts(0), 0).build());
        let _ = Contract::new(accounts(1), U128(0), meta());
    }
}
