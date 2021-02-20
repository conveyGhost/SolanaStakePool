//! Program state processor

use crate::{
    error::StakePoolError,
    instruction::{InitArgs, StakePoolInstruction},
    stake,
    state::{StakePool, ValidatorStakeInfo, ValidatorStakeList},
    PROGRAM_VERSION,
};
use bincode::deserialize;
use num_traits::FromPrimitive;
use solana_program::{
    account_info::next_account_info,
    account_info::AccountInfo,
    clock::Clock,
    decode_error::DecodeError,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::PrintProgramError,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    stake_history::StakeHistory,
    system_instruction,
    sysvar::Sysvar,
};
use spl_token::state::Mint;
use core::convert::{TryFrom,TryInto};

/// Program state handler.
pub struct Processor {}
impl Processor {
    /// Suffix for deposit authority seed
    pub const AUTHORITY_DEPOSIT: &'static [u8] = b"deposit";
    /// Suffix for withdraw authority seed
    pub const AUTHORITY_WITHDRAW: &'static [u8] = b"withdraw";
    /// Calculates the authority id by generating a program address.
    pub fn authority_id(
        program_id: &Pubkey,
        stake_pool: &Pubkey,
        authority_type: &[u8],
        bump_seed: u8,
    ) -> Result<Pubkey, ProgramError> {
        Pubkey::create_program_address(
            &[&stake_pool.to_bytes()[..32], authority_type, &[bump_seed]],
            program_id,
        )
        .map_err(|_| StakePoolError::InvalidProgramAddress.into())
    }
    /// Generates seed bump for stake pool authorities
    pub fn find_authority_bump_seed(
        program_id: &Pubkey,
        stake_pool: &Pubkey,
        authority_type: &[u8],
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[&stake_pool.to_bytes()[..32], authority_type], program_id)
    }
    /// Generates stake account address for the validator
    pub fn find_stake_address_for_validator(
        program_id: &Pubkey,
        validator: &Pubkey,
        stake_pool: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[&validator.to_bytes()[..32], &stake_pool.to_bytes()[..32]],
            program_id,
        )
    }

    /// Checks withdraw or deposit authority
    pub fn check_authority(
        authority_to_check: &Pubkey,
        program_id: &Pubkey,
        stake_pool_key: &Pubkey,
        authority_type: &[u8],
        bump_seed: u8,
    ) -> Result<(), ProgramError> {
        if *authority_to_check
            != Self::authority_id(program_id, stake_pool_key, authority_type, bump_seed)?
        {
            return Err(StakePoolError::InvalidProgramAddress.into());
        }
        Ok(())
    }

    /// Returns validator address for a particular stake account
    pub fn get_validator(stake_account_info: &AccountInfo) -> Result<Pubkey, ProgramError> {
        //deserializes stake-account data
        let stake_state: stake::StakeState = deserialize(&stake_account_info.data.borrow())
            .or(Err(ProgramError::InvalidAccountData))?;
        match stake_state {
            //returns stake.delegation.voter_pubkey
            stake::StakeState::Stake(_, stake) => Ok(stake.delegation.voter_pubkey),
            _ => Err(StakePoolError::WrongStakeState.into()),
        }
    }

    /// Checks if validator stake account is a proper program address
    pub fn is_validator_stake_address(
        validator_account: &Pubkey,
        program_id: &Pubkey,
        stake_pool_info: &AccountInfo,
        stake_account_info: &AccountInfo,
    ) -> bool {
        // Check stake account address validity
        let (stake_address, _) = Self::find_stake_address_for_validator(
            &program_id,
            &validator_account,
            &stake_pool_info.key,
        );
        stake_address == *stake_account_info.key
    }

    /// Returns validator address for a particular stake account and checks its validity
    pub fn get_validator_checked(
        program_id: &Pubkey,
        stake_pool_info: &AccountInfo,
        stake_account_info: &AccountInfo,
    ) -> Result<Pubkey, ProgramError> {
        let validator_account = Self::get_validator(stake_account_info)?;

        if !Self::is_validator_stake_address(
            &validator_account,
            program_id,
            stake_pool_info,
            stake_account_info,
        ) {
            return Err(StakePoolError::InvalidStakeAccountAddress.into());
        }
        Ok(validator_account)
    }

    /// Issue a stake_split instruction.
    #[allow(clippy::too_many_arguments)]
    pub fn stake_split<'a>(
        stake_pool: &Pubkey,
        stake_account: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        authority_type: &[u8],
        bump_seed: u8,
        amount: u64,
        split_stake: AccountInfo<'a>,
        reserved: AccountInfo<'a>,
        stake_program_info: AccountInfo<'a>,
    ) -> Result<(), ProgramError> {
        let me_bytes = stake_pool.to_bytes();
        let authority_signature_seeds = [&me_bytes[..32], authority_type, &[bump_seed]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = stake::split_only(stake_account.key, authority.key, amount, split_stake.key);

        invoke_signed(
            &ix,
            &[
                stake_account,
                reserved,
                authority,
                split_stake,
                stake_program_info,
            ],
            signers,
        )
    }

    /// Issue a stake_merge instruction.
    #[allow(clippy::too_many_arguments)]
    pub fn stake_merge<'a>(
        stake_pool: &Pubkey,
        stake_account: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        authority_type: &[u8],
        bump_seed: u8,
        merge_with: AccountInfo<'a>,
        clock: AccountInfo<'a>,
        stake_history: AccountInfo<'a>,
        stake_program_info: AccountInfo<'a>,
    ) -> Result<(), ProgramError> {
        let me_bytes = stake_pool.to_bytes();
        let authority_signature_seeds = [&me_bytes[..32], authority_type, &[bump_seed]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = stake::merge(merge_with.key, stake_account.key, authority.key);

        invoke_signed(
            &ix,
            &[
                merge_with,
                stake_account,
                clock,
                stake_history,
                authority,
                stake_program_info,
            ],
            signers,
        )
    }

    /// Issue a stake_set_owner instruction.
    #[allow(clippy::too_many_arguments)]
    pub fn stake_authorize<'a>(
        stake_pool: &Pubkey,
        stake_account: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        authority_type: &[u8],
        bump_seed: u8,
        new_staker: &Pubkey,
        staker_auth: stake::StakeAuthorize,
        reserved: AccountInfo<'a>,
        stake_program_info: AccountInfo<'a>,
    ) -> Result<(), ProgramError> {
        let me_bytes = stake_pool.to_bytes();
        let authority_signature_seeds = [&me_bytes[..32], authority_type, &[bump_seed]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = stake::authorize(stake_account.key, authority.key, new_staker, staker_auth);

        invoke_signed(
            &ix,
            &[stake_account, reserved, authority, stake_program_info],
            signers,
        )
    }

    /// Issue a spl_token `Burn` instruction.
    #[allow(clippy::too_many_arguments)]
    pub fn token_burn<'a>(
        stake_pool: &Pubkey,
        token_program: AccountInfo<'a>,
        burn_account: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        authority_type: &[u8],
        bump_seed: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let me_bytes = stake_pool.to_bytes();
        let authority_signature_seeds = [&me_bytes[..32], authority_type, &[bump_seed]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = spl_token::instruction::burn(
            token_program.key,
            burn_account.key,
            mint.key,
            authority.key,
            &[],
            amount,
        )?;

        invoke_signed(
            &ix,
            &[burn_account, mint, authority, token_program],
            signers,
        )
    }

    /// Issue a spl_token `MintTo` instruction.
    #[allow(clippy::too_many_arguments)]
    pub fn token_mint_to<'a>(
        program_id: &Pubkey,
        token_program: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        authority_type: &[u8],
        bump_seed: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let me_bytes = program_id.to_bytes();
        let authority_signature_seeds = [&me_bytes[..32], authority_type, &[bump_seed]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = spl_token::instruction::mint_to(
            token_program.key,
            mint.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;

        invoke_signed(&ix, &[mint, destination, authority, token_program], signers)
    }

    /// Processes `Initialize` instruction.
    pub fn process_initialize(
        program_id: &Pubkey,
        init: InitArgs,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let stake_pool_info = next_account_info(account_info_iter)?;
        let owner_info = next_account_info(account_info_iter)?;
        let validator_stake_list_info = next_account_info(account_info_iter)?;
        let pool_mint_info = next_account_info(account_info_iter)?;
        let owner_fee_info = next_account_info(account_info_iter)?;
        // Clock sysvar account
        let clock_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(clock_info)?;
        // Rent sysvar account
        let rent_info = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(rent_info)?;
        // Token program ID
        let token_program_info = next_account_info(account_info_iter)?;

        // Check if transaction was signed by owner
        if !owner_info.is_signer {
            return Err(StakePoolError::SignatureMissing.into());
        }

        let mut stake_pool = StakePool::deserialize(&stake_pool_info.data.borrow())?;
        // Stake pool account should not be already initialized
        if stake_pool.is_initialized() {
            return Err(StakePoolError::AlreadyInUse.into());
        }

        // Check if validator stake list storage is unitialized
        let mut validator_stake_list =
            ValidatorStakeList::deserialize(&validator_stake_list_info.data.borrow())?;
        if validator_stake_list.is_initialized() {
            return Err(StakePoolError::AlreadyInUse.into());
        }
        validator_stake_list.version = ValidatorStakeList::VALIDATOR_STAKE_LIST_VERSION;
        validator_stake_list.validators.clear();

        // Check if stake pool account is rent-exempt
        if !rent.is_exempt(stake_pool_info.lamports(), stake_pool_info.data_len()) {
            return Err(StakePoolError::AccountNotRentExempt.into());
        }

        // Check if validator stake list account is rent-exempt
        if !rent.is_exempt(
            validator_stake_list_info.lamports(),
            validator_stake_list_info.data_len(),
        ) {
            return Err(StakePoolError::AccountNotRentExempt.into());
        }

        // Numerator should be smaller than or equal to denominator (fee <= 1)
        if init.fee.numerator > init.fee.denominator {
            return Err(StakePoolError::FeeTooHigh.into());
        }

        // Check if fee account's owner the same as token program id
        if owner_fee_info.owner != token_program_info.key {
            return Err(StakePoolError::InvalidFeeAccount.into());
        }

        // Check pool mint program ID
        if pool_mint_info.owner != token_program_info.key {
            return Err(ProgramError::IncorrectProgramId);
        }

        // Check for owner fee account to have proper mint assigned
        if *pool_mint_info.key
            != spl_token::state::Account::unpack_from_slice(&owner_fee_info.data.borrow())?.mint
        {
            return Err(StakePoolError::WrongAccountMint.into());
        }

        let (_, deposit_bump_seed) = Self::find_authority_bump_seed(
            program_id,
            stake_pool_info.key,
            Self::AUTHORITY_DEPOSIT,
        );
        let (withdraw_authority_key, withdraw_bump_seed) = Self::find_authority_bump_seed(
            program_id,
            stake_pool_info.key,
            Self::AUTHORITY_WITHDRAW,
        );

        let pool_mint = Mint::unpack_from_slice(&pool_mint_info.data.borrow())?;

        if !pool_mint.mint_authority.contains(&withdraw_authority_key) {
            return Err(StakePoolError::WrongMintingAuthority.into());
        }

        validator_stake_list.serialize(&mut validator_stake_list_info.data.borrow_mut())?;

        msg!("Clock data: {:?}", clock_info.data.borrow());
        msg!("Epoch: {}", clock.epoch);

        stake_pool.version = PROGRAM_VERSION;
        stake_pool.owner = *owner_info.key;
        stake_pool.deposit_bump_seed = deposit_bump_seed;
        stake_pool.withdraw_bump_seed = withdraw_bump_seed;
        stake_pool.validator_stake_list = *validator_stake_list_info.key;
        stake_pool.pool_mint = *pool_mint_info.key;
        stake_pool.owner_fee_account = *owner_fee_info.key;
        stake_pool.token_program_id = *token_program_info.key;
        stake_pool.last_update_epoch = clock.epoch;
        stake_pool.fee = init.fee;

        stake_pool.serialize(&mut stake_pool_info.data.borrow_mut())
    }

    /// Processes `CreateValidatorStakeAccount` instruction.
    pub fn process_create_validator_stake_account(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        // Stake pool account
        let stake_pool_info = next_account_info(account_info_iter)?;
        // Account creation funder account
        let funder_info = next_account_info(account_info_iter)?;
        // Stake account to be created
        let stake_account_info = next_account_info(account_info_iter)?;
        // Validator this stake account will vote for
        let validator_info = next_account_info(account_info_iter)?;
        // Stake authority for the new stake account
        let stake_authority_info = next_account_info(account_info_iter)?;
        // Withdraw authority for the new stake account
        let withdraw_authority_info = next_account_info(account_info_iter)?;
        // Rent sysvar account
        let rent_info = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(rent_info)?;
        // System program id
        let system_program_info = next_account_info(account_info_iter)?;
        // Staking program id
        let stake_program_info = next_account_info(account_info_iter)?;

        // Check program ids
        if *system_program_info.key != solana_program::system_program::id() {
            return Err(ProgramError::IncorrectProgramId);
        }
        if *stake_program_info.key != stake::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        // Check stake account address validity
        let (stake_address, bump_seed) = Self::find_stake_address_for_validator(
            &program_id,
            &validator_info.key,
            &stake_pool_info.key,
        );
        if stake_address != *stake_account_info.key {
            return Err(StakePoolError::InvalidStakeAccountAddress.into());
        }

        let stake_account_signer_seeds: &[&[_]] = &[
            &validator_info.key.to_bytes()[..32],
            &stake_pool_info.key.to_bytes()[..32],
            &[bump_seed],
        ];

        // Fund the associated token account with the minimum balance to be rent exempt
        let required_lamports = 1 + rent.minimum_balance(std::mem::size_of::<stake::StakeState>());

        // Create new stake account
        invoke_signed(
            &system_instruction::create_account(
                &funder_info.key,
                &stake_account_info.key,
                required_lamports,
                std::mem::size_of::<stake::StakeState>() as u64,
                &stake::id(),
            ),
            &[funder_info.clone(), stake_account_info.clone()],
            &[&stake_account_signer_seeds],
        )?;

        invoke(
            &stake::initialize(
                &stake_account_info.key,
                &stake::Authorized {
                    staker: *stake_authority_info.key,
                    withdrawer: *withdraw_authority_info.key,
                },
                &stake::Lockup::default(),
            ),
            &[
                stake_account_info.clone(),
                rent_info.clone(),
                stake_program_info.clone(),
            ],
        )
    }

    /// Processes `AddValidatorStakeAccount` instruction.
    pub fn process_add_validator_stake_account(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        // Stake pool account
        let stake_pool_info = next_account_info(account_info_iter)?;
        // Pool owner account
        let owner_info = next_account_info(account_info_iter)?;
        // Stake pool deposit authority
        let deposit_info = next_account_info(account_info_iter)?;
        // Stake pool withdraw authority
        let withdraw_info = next_account_info(account_info_iter)?;
        // Account storing validator stake list
        let validator_stake_list_info = next_account_info(account_info_iter)?;
        // Stake account to add to the pool
        let stake_account_info = next_account_info(account_info_iter)?;
        // User account to receive pool tokens
        let dest_user_info = next_account_info(account_info_iter)?;
        // Pool token mint account
        let pool_mint_info = next_account_info(account_info_iter)?;
        // Clock sysvar account
        let clock_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(clock_info)?;
        // Stake history sysvar account
        let stake_history_info = next_account_info(account_info_iter)?;
        let stake_history = &StakeHistory::from_account_info(stake_history_info)?;
        // Pool token program id
        let token_program_info = next_account_info(account_info_iter)?;
        // Staking program id
        let stake_program_info = next_account_info(account_info_iter)?;

        // Check program ids
        if *stake_program_info.key != stake::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        // Get stake pool stake (and check if it is initialized)
        let mut stake_pool_data = StakePool::deserialize(&stake_pool_info.data.borrow())?;
        if !stake_pool_data.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        // Check authority accounts
        stake_pool_data.check_authority_withdraw(withdraw_info.key, program_id, stake_pool_info.key)?;
        stake_pool_data.check_authority_deposit(deposit_info.key, program_id, stake_pool_info.key)?;

        // Check owner validity and signature
        stake_pool_data.check_owner(owner_info)?;

        // Check stake pool last update epoch
        if stake_pool_data.last_update_epoch < clock.epoch {
            return Err(StakePoolError::StakeListAndPoolOutOfDate.into());
        }

        if stake_pool_data.token_program_id != *token_program_info.key {
            return Err(ProgramError::IncorrectProgramId);
        }
        if stake_pool_data.pool_mint != *pool_mint_info.key {
            return Err(StakePoolError::WrongPoolMint.into());
        }

        // Check validator stake account list storage
        if *validator_stake_list_info.key != stake_pool_data.validator_stake_list {
            return Err(StakePoolError::InvalidValidatorStakeList.into());
        }

        // Read validator stake list account and check if it is valid
        let mut validator_stake_list =
            ValidatorStakeList::deserialize(&validator_stake_list_info.data.borrow())?;
        if !validator_stake_list.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        let validator_account =
            Self::get_validator_checked(program_id, stake_pool_info, stake_account_info)?;

        if validator_stake_list.contains(&validator_account) {
            return Err(StakePoolError::ValidatorAlreadyAdded.into());
        }

        // Update Withdrawer and Staker authority to the program withdraw authority
        for authority in &[
            stake::StakeAuthorize::Withdrawer,
            stake::StakeAuthorize::Staker,
        ] {
            Self::stake_authorize(
                stake_pool_info.key,
                stake_account_info.clone(),
                deposit_info.clone(),
                Self::AUTHORITY_DEPOSIT,
                stake_pool_data.deposit_bump_seed,
                withdraw_info.key,
                *authority,
                clock_info.clone(),
                stake_program_info.clone(),
            )?;
        }

        // Calculate and mint tokens
        let stake_lamports = **stake_account_info.lamports.borrow();
        let token_amount = stake_pool_data
            .calc_pool_deposit_amount(stake_lamports)
            .ok_or(StakePoolError::CalculationFailure)?;
        Self::token_mint_to(
            stake_pool_info.key,
            token_program_info.clone(),
            pool_mint_info.clone(),
            dest_user_info.clone(),
            withdraw_info.clone(),
            Self::AUTHORITY_WITHDRAW,
            stake_pool_data.withdraw_bump_seed,
            token_amount,
        )?;

        // Check if stake is warmed up
        Self::check_stake_activation(stake_account_info, clock, stake_history)?;

        // Add validator to the list and save
        validator_stake_list.validators.push(ValidatorStakeInfo {
            validator_account,
            balance: stake_lamports,
            last_update_epoch: clock.epoch,
        });
        validator_stake_list.serialize(&mut validator_stake_list_info.data.borrow_mut())?;

        // Save amounts to the stake pool state
        stake_pool_data.pool_total += token_amount;
        // Only update stake total if the last state update epoch is current
        stake_pool_data.stake_total += stake_lamports;
        stake_pool_data.serialize(&mut stake_pool_info.data.borrow_mut())?;

        Ok(())
    }

    /// Processes `RemoveValidatorStakeAccount` instruction.
    pub fn process_remove_validator_stake_account(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        // Stake pool account
        let stake_pool_info = next_account_info(account_info_iter)?;
        // Pool owner account
        let owner_info = next_account_info(account_info_iter)?;
        // Stake pool withdraw authority
        let withdraw_info = next_account_info(account_info_iter)?;
        // New stake authority
        let new_stake_authority_info = next_account_info(account_info_iter)?;
        // Account storing validator stake list
        let validator_stake_list_info = next_account_info(account_info_iter)?;
        // Stake account to remove from the pool
        let stake_account_info = next_account_info(account_info_iter)?;
        // User account with pool tokens to burn from
        let burn_from_info = next_account_info(account_info_iter)?;
        // Pool token mint account
        let pool_mint_info = next_account_info(account_info_iter)?;
        // Clock sysvar account
        let clock_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(clock_info)?;
        // Pool token program id
        let token_program_info = next_account_info(account_info_iter)?;
        // Staking program id
        let stake_program_info = next_account_info(account_info_iter)?;

        // Check program ids
        if *stake_program_info.key != stake::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        // Get stake pool stake (and check if it is initialized)
        let mut stake_pool = StakePool::deserialize(&stake_pool_info.data.borrow())?;
        if !stake_pool.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        // Check authority account
        stake_pool.check_authority_withdraw(withdraw_info.key, program_id, stake_pool_info.key)?;

        // Check owner validity and signature
        stake_pool.check_owner(owner_info)?;

        // Check stake pool last update epoch
        if stake_pool.last_update_epoch < clock.epoch {
            return Err(StakePoolError::StakeListAndPoolOutOfDate.into());
        }

        if stake_pool.token_program_id != *token_program_info.key {
            return Err(ProgramError::IncorrectProgramId);
        }
        if stake_pool.pool_mint != *pool_mint_info.key {
            return Err(StakePoolError::WrongPoolMint.into());
        }

        // Check validator stake account list storage
        if *validator_stake_list_info.key != stake_pool.validator_stake_list {
            return Err(StakePoolError::InvalidValidatorStakeList.into());
        }

        // Read validator stake list account and check if it is valid
        let mut validator_stake_list =
            ValidatorStakeList::deserialize(&validator_stake_list_info.data.borrow())?;
        if !validator_stake_list.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        let validator_account =
            Self::get_validator_checked(program_id, stake_pool_info, stake_account_info)?;

        if !validator_stake_list.contains(&validator_account) {
            return Err(StakePoolError::ValidatorNotFound.into());
        }

        // Update Withdrawer and Staker authority to the provided authority
        for authority in &[
            stake::StakeAuthorize::Withdrawer,
            stake::StakeAuthorize::Staker,
        ] {
            Self::stake_authorize(
                stake_pool_info.key,
                stake_account_info.clone(),
                withdraw_info.clone(),
                Self::AUTHORITY_WITHDRAW,
                stake_pool.withdraw_bump_seed,
                new_stake_authority_info.key,
                *authority,
                clock_info.clone(),
                stake_program_info.clone(),
            )?;
        }

        // Calculate and burn tokens
        let stake_lamports = **stake_account_info.lamports.borrow();
        let token_amount = stake_pool
            .calc_pool_withdraw_amount(stake_lamports)
            .ok_or(StakePoolError::CalculationFailure)?;
        Self::token_burn(
            stake_pool_info.key,
            token_program_info.clone(),
            burn_from_info.clone(),
            pool_mint_info.clone(),
            withdraw_info.clone(),
            Self::AUTHORITY_WITHDRAW,
            stake_pool.withdraw_bump_seed,
            token_amount,
        )?;

        // Remove validator from the list and save
        validator_stake_list
            .validators
            .retain(|item| item.validator_account != validator_account);
        validator_stake_list.serialize(&mut validator_stake_list_info.data.borrow_mut())?;

        // Save amounts to the stake pool state
        stake_pool.pool_total -= token_amount;
        // Only update stake total if the last state update epoch is current
        stake_pool.stake_total -= stake_lamports;
        stake_pool.serialize(&mut stake_pool_info.data.borrow_mut())?;

        Ok(())
    }

    /// Processes `UpdateListBalance` instruction.
    pub fn process_update_list_balance(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        // Account storing validator stake list
        let validator_stake_list_info = next_account_info(account_info_iter)?;
        // Clock sysvar account
        let clock_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(clock_info)?;
        // Validator stake accounts
        let validator_stake_accounts = account_info_iter.as_slice();

        // Read validator stake list account and check if it is valid
        let mut validator_stake_list =
            ValidatorStakeList::deserialize(&validator_stake_list_info.data.borrow())?;
        if !validator_stake_list.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        let validator_accounts: Vec<Option<Pubkey>> = validator_stake_accounts
            .iter()
            .map(|stake| Self::get_validator(stake).ok())
            .collect();

        let mut changes = false;
        // Do a brute iteration through the list, optimize if necessary
        for validator_stake_record in &mut validator_stake_list.validators {
            if validator_stake_record.last_update_epoch >= clock.epoch {
                continue;
            }
            for (validator_stake_account, validator_account) in validator_stake_accounts
                .iter()
                .zip(validator_accounts.iter())
            {
                if validator_stake_record.validator_account
                    != validator_account.ok_or(StakePoolError::WrongStakeState)?
                {
                    continue;
                }
                validator_stake_record.last_update_epoch = clock.epoch;
                validator_stake_record.balance = **validator_stake_account.lamports.borrow();
                changes = true;
            }
        }

        if changes {
            validator_stake_list.serialize(&mut validator_stake_list_info.data.borrow_mut())?;
        }

        Ok(())
    }

    /// Processes `UpdatePoolBalance` instruction.
    pub fn process_update_pool_balance(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        // Stake pool account
        let stake_pool_info = next_account_info(account_info_iter)?;
        // Account storing validator stake list
        let validator_stake_list_info = next_account_info(account_info_iter)?;
        // Clock sysvar account
        let clock_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(clock_info)?;

        // Get stake pool stake (and check if it is initialized)
        let mut stake_pool = StakePool::deserialize(&stake_pool_info.data.borrow())?;
        if !stake_pool.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        // Check validator stake account list storage
        if *validator_stake_list_info.key != stake_pool.validator_stake_list {
            return Err(StakePoolError::InvalidValidatorStakeList.into());
        }

        // Read validator stake list account and check if it is valid
        let validator_stake_list =
            ValidatorStakeList::deserialize(&validator_stake_list_info.data.borrow())?;
        if !validator_stake_list.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        let mut total_balance: u64 = 0;
        for validator_stake_record in validator_stake_list.validators {
            if validator_stake_record.last_update_epoch < clock.epoch {
                return Err(StakePoolError::StakeListOutOfDate.into());
            }
            total_balance += validator_stake_record.balance;
        }

        stake_pool.stake_total = total_balance;
        stake_pool.last_update_epoch = clock.epoch;
        stake_pool.serialize(&mut stake_pool_info.data.borrow_mut())?;

        Ok(())
    }

    /// Check stake activation status
    pub fn check_stake_activation(
        _stake_info: &AccountInfo,
        _clock: &Clock,
        _stake_history: &StakeHistory,
    ) -> ProgramResult {
        // TODO: remove conditional compilation when time travel in tests is possible
        //#[cfg(not(feature = "test-bpf"))]
        // This check is commented to make tests run without special command line arguments
        /*{
            let stake_acc_state: stake::StakeState =
                deserialize(&stake_info.data.borrow()).unwrap();
            let delegation = stake_acc_state.delegation();
            if let Some(delegation) = delegation {
                let target_epoch = clock.epoch;
                let history = Some(stake_history);
                let fix_stake_deactivate = true;
                let (effective, activating, deactivating) = delegation
                    .stake_activating_and_deactivating(target_epoch, history, fix_stake_deactivate);
                if activating != 0 || deactivating != 0 || effective == 0 {
                    return Err(StakePoolError::UserStakeNotActive.into());
                }
            } else {
                return Err(StakePoolError::WrongStakeState.into());
            }
        }*/
        Ok(())
    }

    /// Unpacks a spl_token `Account`.
    pub fn unpack_token_account(
        account_info: &AccountInfo,
        token_program_id: &Pubkey,

    ) -> Result<spl_token::state::Account, StakePoolError> {

        if account_info.owner != token_program_id {
            Err(StakePoolError::IncorrectTokenProgramId)
        } 
        else {
            spl_token::state::Account::unpack(&account_info.data.borrow())
                .map_err(|_| StakePoolError::ExpectedAccount)
        }
    }

    /// Unpacks a spl_token `Mint`.
    pub fn unpack_mint(
        account_info: &AccountInfo,
        token_program_id: &Pubkey,
    ) -> Result<spl_token::state::Mint, StakePoolError> {
        if account_info.owner != token_program_id {
            Err(StakePoolError::IncorrectTokenProgramId)
        } else {
            spl_token::state::Mint::unpack(&account_info.data.borrow())
                .map_err(|_| StakePoolError::ExpectedMint)
        }
    }
    
    // #[allow(clippy::too_many_arguments)]
    // fn check_accounts(
    //     program_id: &Pubkey,
    //     authority_info: &AccountInfo,
    //     source_account_info: &AccountInfo,
    //     token_b_info: &AccountInfo,
    //     pool_mint_info: &AccountInfo,
    //     token_program_info: &AccountInfo,
    //     user_token_a_info: Option<&AccountInfo>,
    //     user_token_b_info: Option<&AccountInfo>,
    //     pool_fee_account_info: Option<&AccountInfo>,

    // ) -> ProgramResult {

    //     if source_account_info.owner != program_id {
    //         return Err(ProgramError::IncorrectProgramId);
    //     }
    //     if *source_account_info.key != *token_swap.token_a_account() {
    //         return Err(StakePoolError::IncorrectSwapAccount.into());
    //     }
    //     if *token_b_info.key != *token_swap.token_b_account() {
    //         return Err(StakePoolError::IncorrectSwapAccount.into());
    //     }
    //     if *pool_mint_info.key != *token_swap.pool_mint() {
    //         return Err(StakePoolError::IncorrectPoolMint.into());
    //     }
    //     if *token_program_info.key != *token_swap.token_program_id() {
    //         return Err(StakePoolError::IncorrectTokenProgramId.into());
    //     }
    //     if let Some(user_token_a_info) = user_token_a_info {
    //         if source_account_info.key == user_token_a_info.key {
    //             return Err(StakePoolError::InvalidInput.into());
    //         }
    //     }
    //     if let Some(user_token_b_info) = user_token_b_info {
    //         if token_b_info.key == user_token_b_info.key {
    //             return Err(StakePoolError::InvalidInput.into());
    //         }
    //     }
    //     if let Some(pool_fee_account_info) = pool_fee_account_info {
    //         if *pool_fee_account_info.key != *token_swap.pool_fee_account() {
    //             return Err(StakePoolError::IncorrectFeeAccount.into());
    //         }
    //     }
    //     Ok(())
    // }

    /// Issue a spl_token `Transfer` instruction.
    pub fn token_transfer<'a>(
        owner: &Pubkey,
        token_program: AccountInfo<'a>,
        source: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        nonce: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let swap_bytes = owner.to_bytes();
        let authority_signature_seeds = [&swap_bytes[..32], &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;
        invoke_signed(
            &ix,
            &[source, destination, authority, token_program],
            signers,
        )
    }
    
    /// Processes [AddLiquidity](enum.Instruction.html).
    pub fn process_add_liquidity(

        program_id: &Pubkey,
        wsol_amount: u64,
        accounts: &[AccountInfo],

    ) -> ProgramResult {

        if wsol_amount == 0 {
            return Err(StakePoolError::ZeroAmount.into());
        }

        let account_info_iter = &mut accounts.iter();

        // Stake pool account
        let stake_pool_info = next_account_info(account_info_iter)?;

        let token_program = next_account_info(account_info_iter)?;
        let metalp_token_mint_account = next_account_info(account_info_iter)?;

        let metalp_mint_withdraw_authority = next_account_info(account_info_iter)?;

        let user_wsol_source_account = next_account_info(account_info_iter)?;
        let user_transfer_authority_info = next_account_info(account_info_iter)?;

        let liq_pool_wsol_dest_account = next_account_info(account_info_iter)?;
        let user_metalp_account_destination = next_account_info(account_info_iter)?;

        // Get stake pool stake (and check if it is initialized)
        let stake_pool_data = StakePool::deserialize(&stake_pool_info.data.borrow())?;
        if !stake_pool_data.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        //get data from spl-token accounts
        //let source_account_info: spl_token::state::Account = Self::unpack_token_account(user_wsol_source_account, &program_id)?;
        //let dest_user_account_info: spl_token::state::Account = Self::unpack_token_account(user_metalp_account_destination, &program_id)?;

        // TODO
        // Self::check_accounts(
        //     program_id,
        //     authority_info,
        //     user_wsol_source_account,
        //     liq_pool_wsol_dest_account,
        //     metalp_token_mint_account,
        //     liq_pool_wsol_dest_account,
        // )?;

        let metalp_mint_info = Self::unpack_mint(metalp_token_mint_account, &program_id)?;
        let metalp_supply = to_u128(metalp_mint_info.supply)?;

        let our_wsol_token_info = Self::unpack_token_account(liq_pool_wsol_dest_account, &program_id)?;
        let our_wsol_total = to_u128(our_wsol_token_info.amount)?;

        //transfer user wsol to our wsol account
        Self::token_transfer(
            user_transfer_authority_info.key,
            token_program.clone(),
            user_wsol_source_account.clone(),
            liq_pool_wsol_dest_account.clone(),
            user_transfer_authority_info.clone(),
            0,
            wsol_amount,
        )?;

        // Calculate metalp amount and mint metalp tokens for the user
        let metalp_amount = shares_from_value(wsol_amount, our_wsol_total, metalp_supply).ok_or(StakePoolError::CalculationFailure)?;
        Self::token_mint_to(
            &program_id,
            token_program.clone(),
            metalp_token_mint_account.clone(),
            user_metalp_account_destination.clone(),
            metalp_mint_withdraw_authority.clone(),
            Self::AUTHORITY_WITHDRAW,
            0,
            metalp_amount,
        )?;

        Ok(())
    }

    
    /// Processes [Deposit](enum.Instruction.html).
    pub fn process_deposit(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        // Stake pool
        let stake_pool_info = next_account_info(account_info_iter)?;
        // Account storing validator stake list
        let validator_stake_list_info = next_account_info(account_info_iter)?;
        // Stake pool deposit authority
        let deposit_info = next_account_info(account_info_iter)?;
        // Stake pool withdraw authority
        let withdraw_info = next_account_info(account_info_iter)?;
        // Stake account to join the pool (withdraw should be set to stake pool deposit)
        let stake_info = next_account_info(account_info_iter)?;
        // Validator stake account to merge with
        let validator_stake_account_info = next_account_info(account_info_iter)?;
        // User account to receive pool tokens
        let dest_user_info = next_account_info(account_info_iter)?;
        // Account to receive pool fee tokens
        let owner_fee_info = next_account_info(account_info_iter)?;
        // Pool token mint account
        let pool_mint_info = next_account_info(account_info_iter)?;
        // Clock sysvar account
        let clock_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(clock_info)?;
        // Stake history sysvar account
        let stake_history_info = next_account_info(account_info_iter)?;
        let stake_history = &StakeHistory::from_account_info(stake_history_info)?;
        // Pool token program id
        let token_program_info = next_account_info(account_info_iter)?;
        // Stake program id
        let stake_program_info = next_account_info(account_info_iter)?;

        // Check program ids
        if *stake_program_info.key != stake::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        let mut stake_pool = StakePool::deserialize(&stake_pool_info.data.borrow())?;
        if !stake_pool.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        // Check if stake is active
        Self::check_stake_activation(stake_info, clock, stake_history)?;

        // Check authority accounts
        stake_pool.check_authority_withdraw(withdraw_info.key, program_id, stake_pool_info.key)?;
        stake_pool.check_authority_deposit(deposit_info.key, program_id, stake_pool_info.key)?;

        if stake_pool.owner_fee_account != *owner_fee_info.key {
            return Err(StakePoolError::InvalidFeeAccount.into());
        }
        if stake_pool.token_program_id != *token_program_info.key {
            return Err(ProgramError::IncorrectProgramId);
        }

        // Check validator stake account list storage
        if *validator_stake_list_info.key != stake_pool.validator_stake_list {
            return Err(StakePoolError::InvalidValidatorStakeList.into());
        }

        // Check stake pool last update epoch
        if stake_pool.last_update_epoch < clock.epoch {
            return Err(StakePoolError::StakeListAndPoolOutOfDate.into());
        }

        // Read validator stake list account and check if it is valid
        let mut validator_stake_list =
            ValidatorStakeList::deserialize(&validator_stake_list_info.data.borrow())?;
        if !validator_stake_list.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        let validator_account =
            Self::get_validator_checked(program_id, stake_pool_info, validator_stake_account_info)?;

        //fin the validator account in the validator list
        let validator_list_item = validator_stake_list
            .find_mut(&validator_account)
            .ok_or(StakePoolError::ValidatorNotFound)?;

        // take the amount form the account created by the user to stake
        let stake_lamports = **stake_info.lamports.borrow();
        // computes how many shares/tokens of the pool the amount being staked represents
        // ("pool" here means "tokens")
        let pool_amount = stake_pool
            .calc_pool_deposit_amount(stake_lamports)
            .ok_or(StakePoolError::CalculationFailure)?;

        // apply fee% to token amount
        let fee_amount = stake_pool
            .calc_fee_amount(pool_amount)
            .ok_or(StakePoolError::CalculationFailure)?;

        // discount fee% from token amount
        let user_amount = pool_amount
            .checked_sub(fee_amount)
            .ok_or(StakePoolError::CalculationFailure)?;

        // sets stake_pool_info as "Withdrawer" for stake_info
        Self::stake_authorize(
            stake_pool_info.key,
            stake_info.clone(),
            deposit_info.clone(),
            Self::AUTHORITY_DEPOSIT,
            stake_pool.deposit_bump_seed,
            withdraw_info.key,
            stake::StakeAuthorize::Withdrawer,
            clock_info.clone(),
            stake_program_info.clone(),
        )?;

        // sets stake_pool_info as "Staker" for stake_info
        Self::stake_authorize(
            stake_pool_info.key,
            stake_info.clone(),
            deposit_info.clone(),
            Self::AUTHORITY_DEPOSIT,
            stake_pool.deposit_bump_seed,
            withdraw_info.key,
            stake::StakeAuthorize::Staker,
            clock_info.clone(),
            stake_program_info.clone(),
        )?;

        // merges stake_info into stake_pool_info
        Self::stake_merge(
            stake_pool_info.key,
            stake_info.clone(),
            withdraw_info.clone(),
            Self::AUTHORITY_WITHDRAW,
            stake_pool.withdraw_bump_seed,
            validator_stake_account_info.clone(),
            clock_info.clone(),
            stake_history_info.clone(),
            stake_program_info.clone(),
        )?;

        // mints tokens/shares for the user (tokens minus fee)
        Self::token_mint_to(
            stake_pool_info.key,
            token_program_info.clone(),
            pool_mint_info.clone(),
            dest_user_info.clone(),
            withdraw_info.clone(),
            Self::AUTHORITY_WITHDRAW,
            stake_pool.withdraw_bump_seed,
            user_amount,
        )?;

        // mints *fee* tokens/shares for owner_fee_info
        Self::token_mint_to(
            stake_pool_info.key,
            token_program_info.clone(),
            pool_mint_info.clone(),
            owner_fee_info.clone(),
            withdraw_info.clone(),
            Self::AUTHORITY_WITHDRAW,
            stake_pool.withdraw_bump_seed,
            fee_amount,
        )?;
        //update total tokens/shares 
        stake_pool.pool_total += pool_amount;
        //update total staked
        stake_pool.stake_total += stake_lamports;
        //save contract state into stake_pool_info account
        stake_pool.serialize(&mut stake_pool_info.data.borrow_mut())?;

        //update validator balance in out internal list
        validator_list_item.balance = **validator_stake_account_info.lamports.borrow();
        //save updated validator list into validator_stake_list_info account
        validator_stake_list.serialize(&mut validator_stake_list_info.data.borrow_mut())?;

        Ok(())
    }

    /// Processes [Withdraw](enum.Instruction.html).
    /// split contract's staking acc into stake_split_to and assigns authority to user_stake_authority
    pub fn process_withdraw(
        program_id: &Pubkey,
        pool_amount: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        // Stake pool
        let stake_pool_info = next_account_info(account_info_iter)?;
        // Account storing validator stake list
        let validator_stake_list_info = next_account_info(account_info_iter)?;
        // Stake pool withdraw authority
        let withdraw_info = next_account_info(account_info_iter)?;
        // Stake account to split
        let stake_split_from = next_account_info(account_info_iter)?;
        // Unitialized stake account to receive withdrawal
        let stake_split_to = next_account_info(account_info_iter)?;
        // User account to set as a new withdraw authority
        let user_stake_authority = next_account_info(account_info_iter)?;
        // User account with pool tokens to burn from
        let burn_from_info = next_account_info(account_info_iter)?;
        // Pool token mint account
        let pool_mint_info = next_account_info(account_info_iter)?;
        // Clock sysvar account
        let clock_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(clock_info)?;
        // Pool token program id
        let token_program_info = next_account_info(account_info_iter)?;
        // Stake program id
        let stake_program_info = next_account_info(account_info_iter)?;

        // Check program ids
        if *stake_program_info.key != stake::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        // load contract state
        let mut stake_pool = StakePool::deserialize(&stake_pool_info.data.borrow())?;
        if !stake_pool.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        // Check authority account
        stake_pool.check_authority_withdraw(withdraw_info.key, program_id, stake_pool_info.key)?;

        if stake_pool.token_program_id != *token_program_info.key {
            return Err(ProgramError::IncorrectProgramId);
        }

        // Check validator stake account list storage
        if *validator_stake_list_info.key != stake_pool.validator_stake_list {
            return Err(StakePoolError::InvalidValidatorStakeList.into());
        }

        // Check stake pool last update epoch
        if stake_pool.last_update_epoch < clock.epoch {
            return Err(StakePoolError::StakeListAndPoolOutOfDate.into());
        }

        // Read validator stake list account and check if it is valid
        let mut validator_stake_list =
            ValidatorStakeList::deserialize(&validator_stake_list_info.data.borrow())?;
        if !validator_stake_list.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        //get the validator pub key from the state stored at stake_pool_info arg
        let validator_account =
            Self::get_validator_checked(program_id, stake_pool_info, stake_split_from)?;

        //finds validator in our internal list
        let validator_list_item = validator_stake_list
            .find_mut(&validator_account)
            .ok_or(StakePoolError::ValidatorNotFound)?;

        // computes how many lamports represent the token/shares being sold|burned
        let stake_amount = stake_pool
            .calc_lamports_amount(pool_amount)
            .ok_or(StakePoolError::CalculationFailure)?;

        // split the amount from this contract stake acc into stake_split_to
        Self::stake_split(
            stake_pool_info.key,
            stake_split_from.clone(),
            withdraw_info.clone(),
            Self::AUTHORITY_WITHDRAW,
            stake_pool.withdraw_bump_seed,
            stake_amount,
            stake_split_to.clone(),
            clock_info.clone(),
            stake_program_info.clone(),
        )?;

        // "transfer" the mount to the user in a convoluted solana-requeried way:
        // 1. splits the staked amount *into a new account* the caller created just for this call and sent as parameter
        // 2. moves ownership of the new account (who can withdraw, who can stake, to the user)
        // at the end, the user "new account" has the lamports, and the user can operate it.- 
        //
        //assigns Withdrawer to user on the split account
        Self::stake_authorize(
            stake_pool_info.key,
            stake_split_to.clone(),
            withdraw_info.clone(),
            Self::AUTHORITY_WITHDRAW,
            stake_pool.withdraw_bump_seed,
            user_stake_authority.key,
            stake::StakeAuthorize::Withdrawer,
            clock_info.clone(),
            stake_program_info.clone(),
        )?;

        //assings Staker to user on the split account
        Self::stake_authorize(
            stake_pool_info.key,
            stake_split_to.clone(),
            withdraw_info.clone(),
            Self::AUTHORITY_WITHDRAW,
            stake_pool.withdraw_bump_seed,
            user_stake_authority.key,
            stake::StakeAuthorize::Staker,
            clock_info.clone(),
            stake_program_info.clone(),
        )?;

        //burns the tokens
        Self::token_burn(
            stake_pool_info.key,
            token_program_info.clone(),
            burn_from_info.clone(),
            pool_mint_info.clone(),
            withdraw_info.clone(),
            Self::AUTHORITY_WITHDRAW,
            stake_pool.withdraw_bump_seed,
            pool_amount,
        )?;

        //update token supply
        stake_pool.pool_total -= pool_amount;
        //update total staked
        stake_pool.stake_total -= stake_amount;
        //save into contract state
        stake_pool.serialize(&mut stake_pool_info.data.borrow_mut())?;

        //updte our internal validator list
        validator_list_item.balance = **stake_split_from.lamports.borrow();
        //save into validator_stake_list state
        validator_stake_list.serialize(&mut validator_stake_list_info.data.borrow_mut())?;

        Ok(())
    }
    /// Processes [SetStakeAuthority](enum.Instruction.html).
    pub fn process_set_staking_auth(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let stake_pool_info = next_account_info(account_info_iter)?;
        let owner_info = next_account_info(account_info_iter)?;
        let withdraw_info = next_account_info(account_info_iter)?;
        let stake_info = next_account_info(account_info_iter)?;
        let staker_info = next_account_info(account_info_iter)?;
        // (Reserved)
        let reserved = next_account_info(account_info_iter)?;
        // Stake program id
        let stake_program_info = next_account_info(account_info_iter)?;

        // Check program ids
        if *stake_program_info.key != stake::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        let stake_pool = StakePool::deserialize(&stake_pool_info.data.borrow())?;
        if !stake_pool.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        // Check authority account
        stake_pool.check_authority_withdraw(withdraw_info.key, program_id, stake_pool_info.key)?;

        // Check owner validity and signature
        stake_pool.check_owner(owner_info)?;

        Self::stake_authorize(
            stake_pool_info.key,
            stake_info.clone(),
            withdraw_info.clone(),
            Self::AUTHORITY_WITHDRAW,
            stake_pool.withdraw_bump_seed,
            staker_info.key,
            stake::StakeAuthorize::Staker,
            reserved.clone(),
            stake_program_info.clone(),
        )?;
        Ok(())
    }

    /// Processes [SetOwner](enum.Instruction.html).
    pub fn process_set_owner(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let stake_pool_info = next_account_info(account_info_iter)?;
        let owner_info = next_account_info(account_info_iter)?;
        let new_owner_info = next_account_info(account_info_iter)?;
        let new_owner_fee_info = next_account_info(account_info_iter)?;

        let mut stake_pool = StakePool::deserialize(&stake_pool_info.data.borrow())?;
        if !stake_pool.is_initialized() {
            return Err(StakePoolError::InvalidState.into());
        }

        // Check owner validity and signature
        stake_pool.check_owner(owner_info)?;

        // Check for owner fee account to have proper mint assigned
        if stake_pool.pool_mint
            != spl_token::state::Account::unpack_from_slice(&new_owner_fee_info.data.borrow())?.mint
        {
            return Err(StakePoolError::WrongAccountMint.into());
        }

        stake_pool.owner = *new_owner_info.key;
        stake_pool.owner_fee_account = *new_owner_fee_info.key;
        stake_pool.serialize(&mut stake_pool_info.data.borrow_mut())?;
        Ok(())
    }
    /// Processes [Instruction](enum.Instruction.html).
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = StakePoolInstruction::deserialize(input)?;
        match instruction {
            StakePoolInstruction::Initialize(init) => {
                msg!("Instruction: Init");
                Self::process_initialize(program_id, init, accounts)
            }
            StakePoolInstruction::CreateValidatorStakeAccount => {
                msg!("Instruction: CreateValidatorStakeAccount");
                Self::process_create_validator_stake_account(program_id, accounts)
            }
            StakePoolInstruction::AddValidatorStakeAccount => {
                msg!("Instruction: AddValidatorStakeAccount");
                Self::process_add_validator_stake_account(program_id, accounts)
            }
            StakePoolInstruction::RemoveValidatorStakeAccount => {
                msg!("Instruction: RemoveValidatorStakeAccount");
                Self::process_remove_validator_stake_account(program_id, accounts)
            }
            StakePoolInstruction::UpdateListBalance => {
                msg!("Instruction: UpdateListBalance");
                Self::process_update_list_balance(program_id, accounts)
            }
            StakePoolInstruction::UpdatePoolBalance => {
                msg!("Instruction: UpdatePoolBalance");
                Self::process_update_pool_balance(program_id, accounts)
            }
            StakePoolInstruction::Deposit => {
                msg!("Instruction: Deposit");
                Self::process_deposit(program_id, accounts)
            }
            StakePoolInstruction::Withdraw(amount) => {
                msg!("Instruction: Withdraw");
                Self::process_withdraw(program_id, amount, accounts)
            }
            StakePoolInstruction::SetStakingAuthority => {
                msg!("Instruction: SetStakingAuthority");
                Self::process_set_staking_auth(program_id, accounts)
            }
            StakePoolInstruction::SetOwner => {
                msg!("Instruction: SetOwner");
                Self::process_set_owner(program_id, accounts)
            }
            StakePoolInstruction::AddLiquidity(wsol_amount) => {
                msg!("Instruction: AddLiquidity");
                Self::process_add_liquidity(program_id, wsol_amount, accounts)
            }
        }
    }
}

impl PrintProgramError for StakePoolError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            StakePoolError::AlreadyInUse => msg!("Error: The account cannot be initialized because it is already being used"),
            StakePoolError::InvalidProgramAddress => msg!("Error: The program address provided doesn't match the value generated by the program"),
            StakePoolError::InvalidState => msg!("Error: The stake pool state is invalid"),
            StakePoolError::CalculationFailure => msg!("Error: The calculation failed"),
            StakePoolError::FeeTooHigh => msg!("Error: Stake pool fee > 1"),
            StakePoolError::WrongAccountMint => msg!("Error: Token account is associated with the wrong mint"),
            StakePoolError::NonZeroBalance => msg!("Error: Account balance should be zero"),
            StakePoolError::WrongOwner => msg!("Error: Wrong pool owner account"),
            StakePoolError::SignatureMissing => msg!("Error: Required signature is missing"),
            StakePoolError::InvalidValidatorStakeList => msg!("Error: Invalid validator stake list account"),
            StakePoolError::InvalidFeeAccount => msg!("Error: Invalid owner fee account"),
            StakePoolError::WrongPoolMint => msg!("Error: Specified pool mint account is wrong"),
            StakePoolError::WrongStakeState => msg!("Error: Stake account is not in the state expected by the program"),
            StakePoolError::UserStakeNotActive => msg!("Error: User stake is not active"),
            StakePoolError::ValidatorAlreadyAdded => msg!("Error: Stake account voting for this validator already exists in the pool"),
            StakePoolError::ValidatorNotFound => msg!("Error: Stake account for this validator not found in the pool"),
            StakePoolError::InvalidStakeAccountAddress => msg!("Error: Stake account address not properly derived from the validator address"),
            StakePoolError::StakeListOutOfDate => msg!("Error: Identify validator stake accounts with old balances and update them"),
            StakePoolError::StakeListAndPoolOutOfDate => msg!("Error: First update old validator stake account balances and then pool stake balance"),
            StakePoolError::UnknownValidatorStakeAccount => {msg!("Error: Validator stake account is not found in the list storage")}
            StakePoolError::WrongMintingAuthority => msg!("Error: Wrong minting authority set for mint pool account"),
            StakePoolError::AccountNotRentExempt => msg!("Error: Account is not rent-exempt"),
            StakePoolError::IncorrectTokenProgramId=> msg!("Error: IncorrectTokenProgramId"),
            StakePoolError::ExpectedMint=> msg!("Error: Expected a Mint Account"),
            StakePoolError::ExpectedAccount=> msg!("Error: Expected an Account"),
            StakePoolError::ZeroAmount=> msg!("Error: Amount must greater than zero"),
            StakePoolError::ConversionFailure=> msg!("Error: Data Conversion Failure"),
        }
    }
}

fn to_u128(val: u64) -> Result<u128, StakePoolError> {
    val.try_into().map_err(|_| StakePoolError::ConversionFailure)
}

fn value_from_shares(shares: u64, total_value:u128, total_shares:u128) -> Option<u64> {
    return proportional(shares, total_value,total_shares);
}
fn shares_from_value(value: u64, total_value:u128, total_shares:u128) -> Option<u64> {
    return proportional(value, total_shares,total_value);
}

/// calculate amount*numerator/denominator
/// as value  = shares * share_price where share_price=total_value/total_shares
/// or shares = amount_value / share_price where share_price=total_value/total_shares 
///     => shares = amount_value * 1/share_price where 1/share_price=total_shares/total_value
fn proportional(amount: u64, numerator:u128, denominator:u128) -> Option<u64> {
    if denominator == 0 { return Some(amount); }
    u64::try_from(
        (amount as u128)
            .checked_mul(numerator)?
            .checked_div(denominator)?,
    )
    .ok()
}
