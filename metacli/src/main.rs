// CLI VERSION 
// but fixing the stake-pool program_id
// and stSOL token_id

// Solana base programs
const W_SOL_1111111_MINT_ACCOUNT:&str="So11111111111111111111111111111111111111112";
const ST_SOL_MINT_ACCOUNT:&str="21ofzqmgounc8bX4CK6j3Ff4zjvX6GmRykUnJAU96zKz";

//Values for TESTNET
const SMART_POOL_PROGRAM_ACCOUNT_ID:&str="E2wGYXEPw46FdJWL1MfRoN3JhQY4w6Dmaz9s4ehV2483";
/*
Creating mint 21ofzqmgounc8bX4CK6j3Ff4zjvX6GmRykUnJAU96zKz
Creating pool fee collection account EG42YnCDMx1ham3NVryGM71yiCo3zNSyJ1ktPvguFtqE
Creating stake pool C3WQybyZc45bhRP4PJnM7JhKQFXmqQR5eWr8n8Lxjgex
Signature: 5AdVComuwVXbcyD2MmXNMgAKGx7aE1o8eXXzCgvmxjdgoPmr7Hd5xL28NARTH88PGbGb3ZzxbkjYKcpgic5fUDTY
*/
//const POOL_FEE_COLLECTION_ACCOUNT:&str="EG42YnCDMx1ham3NVryGM71yiCo3zNSyJ1ktPvguFtqE";
const STAKE_POOL_STATE_ACCOUNT:&str="C3WQybyZc45bhRP4PJnM7JhKQFXmqQR5eWr8n8Lxjgex";

/*
lucio@lucio-kde:~/repos/StakePool/metacli$ ./meta create-liq-pool
Creating meta_lp mint 3XrStMayUhNpsFJzmKyynC99fs1Ppbenpj3kpC77EQEh
Creating liquidity pool BVZhLqnZjG4oxTXJTJZkKtVNJKPJbcZHH2S26KL1o2u3
Creating the 2 sides of the pool, wsol & st_sol accounts 
liquidity pool wsol account: J5Sc1zsbh5qcbfBLABCf7JUTL1TAERWH4v56gyro12mX
liquidity pool st_sol account: 8e4KsbpfBrhPne1AciEi8HidAMeKXGTYhMcEqRYScc2a
*/
/// mint for META_LP
const META_LP_MINT_ACCOUNT:&str="3XrStMayUhNpsFJzmKyynC99fs1Ppbenpj3kpC77EQEh"; 
/// state acc for the liq pool
const LIQ_POOL_ACCOUNT:&str="rxTBFFRfwcgx5YedbwLcKntCwMs9tJoQvzYmRnbpLKS"; 
/// liq pool legs wSOL & stSOL, spl-token accounts
const LIQ_POOL_WSOL_ACCOUNT:&str="J5Sc1zsbh5qcbfBLABCf7JUTL1TAERWH4v56gyro12mX"; 
#[allow(dead_code)]
const LIQ_POOL_ST_SOL_ACCOUNT:&str="8e4KsbpfBrhPne1AciEi8HidAMeKXGTYhMcEqRYScc2a"; 

#[macro_use]
extern crate lazy_static;
use bincode::deserialize;
use clap::{
    crate_description, crate_name, crate_version, value_t, value_t_or_exit, App, AppSettings, Arg,
    ArgGroup, SubCommand,
};
use solana_account_decoder::UiAccountEncoding;
use solana_clap_utils::{
    input_parsers::pubkey_of,
    input_validators::{is_amount, is_keypair,  is_pubkey, is_url}, //is_parsable,
    keypair::signer_from_path
};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::RpcAccountInfoConfig,
    rpc_config::RpcProgramAccountsConfig,
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use solana_program::{instruction::Instruction, program_pack::Pack, pubkey::Pubkey};
use solana_sdk::{
    account::Account,
    commitment_config::CommitmentConfig,
    native_token::*,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_stake_pool::{
    instruction::{
        add_validator_stake_account, create_validator_stake_account, deposit, //
         remove_validator_stake_account, set_owner, //initialize as initialize_pool,
        set_staking_authority, update_list_balance, update_pool_balance, withdraw, //, Fee as PoolFee,
        instruction_add_liquidity, instruction_sell_stsol
        //InitArgs as PoolInitArgs,
    },
    processor::Processor as PoolProcessor,
    stake::authorize as authorize_stake,
    stake::id as stake_program_id,
    stake::StakeAuthorize,
    stake::StakeState,
    state::StakePool,
    state::ValidatorStakeList,
};
use spl_token::{
    self, instruction::approve as approve_token, instruction::initialize_account as initialize_token_account,
    instruction::initialize_mint, native_mint, state::Account as TokenAccount,
    state::Mint as TokenMint,
};
use std::process::exit;

struct Config {
    rpc_client: RpcClient,
    verbose: bool,
    owner: Box<dyn Signer>,
    fee_payer: Box<dyn Signer>,
    commitment_config: CommitmentConfig,

    smart_pool_program_id: Pubkey,
    stake_pool_state_account: Pubkey,

    liq_pool_state_account: Pubkey,
    pda_liq_pool_authority: Pubkey,
    meta_lp_mint_account: Pubkey,
    st_sol_mint_account: Pubkey,
    liq_pool_wsol_account: Pubkey,
    liq_pool_stsol_account: Pubkey,
}

type Error = Box<dyn std::error::Error>;
type CommandResult = Result<Option<Transaction>, Error>;

const STAKE_STATE_LEN: usize = 200;
const MAX_ACCOUNTS_TO_UPDATE: usize = 10;
lazy_static! {
    static ref MIN_STAKE_BALANCE: u64 = sol_to_lamports(1.0);
}

macro_rules! unique_signers {
    ($vec:ident) => {
        $vec.sort_by_key(|l| l.pubkey());
        $vec.dedup();
    };
}

fn pubkey_from(str_pubkey:&str)->Pubkey { String::from(str_pubkey).parse().unwrap()}

fn check_fee_payer_balance(config: &Config, required_balance: u64) -> Result<(), Error> {
    let balance = config.rpc_client.get_balance(&config.fee_payer.pubkey())?;
    if balance < required_balance {
        Err(format!(
            "Fee payer, {}, has insufficient balance: {} required, {} available",
            config.fee_payer.pubkey(),
            lamports_to_sol(required_balance),
            lamports_to_sol(balance)
        )
        .into())
    } else {
        Ok(())
    }
}

fn get_authority_accounts(config: &Config, authority: &Pubkey) -> Vec<(Pubkey, Account)> {
    config
        .rpc_client
        .get_program_accounts_with_config(
            &stake_program_id(),
            RpcProgramAccountsConfig {
                filters: Some(vec![RpcFilterType::Memcmp(Memcmp {
                    offset: 44, // 44 is Withdrawer authority offset in stake accoun stake
                    bytes: MemcmpEncodedBytes::Binary(
                        bs58::encode(authority.to_bytes()).into_string(),
                    ),
                    encoding: None,
                })]),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(UiAccountEncoding::Base64),
                    ..RpcAccountInfoConfig::default()
                },
            },
        )
        .unwrap()
}

fn _check_owner_balance(config: &Config, required_balance: u64) -> Result<(), Error> {
    let balance = config.rpc_client.get_balance(&config.owner.pubkey())?;
    if balance < required_balance {
        Err(format!(
            "Owner, {}, has insufficient balance: {} required, {} available",
            config.owner.pubkey(),
            lamports_to_sol(required_balance),
            lamports_to_sol(balance)
        )
        .into())
    } else {
        Ok(())
    }
}

/*
fn command_create_pool(config: &Config, fee: PoolFee) -> CommandResult {
    let mint_account = Keypair::new();
    println!("Creating mint {}", mint_account.pubkey());

    let pool_fee_account = Keypair::new();
    println!(
        "Creating pool fee collection account {}",
        pool_fee_account.pubkey()
    );

    let pool_account = Keypair::new();
    println!("Creating stake pool {}", pool_account.pubkey());

    let validator_stake_list = Keypair::new();

    let mint_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenMint::LEN)?;
    let pool_fee_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)?;
    let pool_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(StakePool::LEN)?;
    let validator_stake_list_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(ValidatorStakeList::LEN)?;
    let total_rent_free_balances = mint_account_balance
        + pool_fee_account_balance
        + pool_account_balance
        + validator_stake_list_balance;

    let default_decimals = native_mint::DECIMALS;

    // Calculate withdraw authority used for minting pool tokens
    let (withdraw_authority, _) = PoolProcessor::find_authority_bump_seed(
        &spl_stake_pool::id(),
        &pool_account.pubkey(),
        PoolProcessor::AUTHORITY_WITHDRAW,
    );

    if config.verbose {
        println!("Stake pool withdraw authority {}", withdraw_authority);
    }

    let mut transaction = Transaction::new_with_payer(
        &[
            // Account for the stake pool mint
            system_instruction::create_account(
                &config.fee_payer.pubkey(),
                &mint_account.pubkey(),
                mint_account_balance,
                TokenMint::LEN as u64,
                &spl_token::id(),
            ),
            // Account for the pool fee accumulation
            system_instruction::create_account(
                &config.fee_payer.pubkey(),
                &pool_fee_account.pubkey(),
                pool_fee_account_balance,
                TokenAccount::LEN as u64,
                &spl_token::id(),
            ),
            // Account for the stake pool
            system_instruction::create_account(
                &config.fee_payer.pubkey(),
                &pool_account.pubkey(),
                pool_account_balance,
                StakePool::LEN as u64,
                &spl_stake_pool::id(),
            ),
            // Validator stake account list storage
            system_instruction::create_account(
                &config.fee_payer.pubkey(),
                &validator_stake_list.pubkey(),
                validator_stake_list_balance,
                ValidatorStakeList::LEN as u64,
                &spl_stake_pool::id(),
            ),
            // Initialize pool token mint account
            initialize_mint(
                &spl_token::id(),
                &mint_account.pubkey(),
                &withdraw_authority,
                None,
                default_decimals,
            )?,
            // Initialize fee receiver account
            initialize_token_account(
                &spl_token::id(),
                &pool_fee_account.pubkey(),
                &mint_account.pubkey(),
                &config.owner.pubkey(),
            )?,
            // Initialize stake pool account
            initialize_pool(
                &spl_stake_pool::id(),
                &pool_account.pubkey(),
                &config.owner.pubkey(),
                &validator_stake_list.pubkey(),
                &mint_account.pubkey(),
                &pool_fee_account.pubkey(),
                &spl_token::id(),
                PoolInitArgs { fee },
            )?,
        ],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        total_rent_free_balances + fee_calculator.calculate_fee(&transaction.message()),
    )?;
    let mut signers = vec![
        config.fee_payer.as_ref(),
        &pool_account,
        &validator_stake_list,
        &mint_account,
        &pool_fee_account,
        config.owner.as_ref(),
    ];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}
*/

fn command_create_liquidity_pool(config: &Config) -> CommandResult {

    let st_sol_mint_account = Keypair::new();
    println!("Creating meta_lp mint {}", st_sol_mint_account.pubkey());

    // let pool_fee_account = Keypair::new();
    // println!(
    //     "Creating pool fee collection account {}",
    //     pool_fee_account.pubkey()
    // );

    let liq_pool_account = Keypair::new();
    println!("Creating liquidity pool {}", liq_pool_account.pubkey());

    //let validator_stake_list = Keypair::new();

    let mint_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenMint::LEN)?;

    let pool_fee_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)?;

    let pool_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(StakePool::LEN)?;

    let total_rent_free_balances = mint_account_balance
        + pool_fee_account_balance
        + pool_account_balance;

    let default_decimals = native_mint::DECIMALS;

    if config.verbose {
        println!("liq pool state acc {}", &config.liq_pool_state_account);
        println!("liq pool PDA authority {}", &config.pda_liq_pool_authority);
    }

    println!(
        "Creating the 2 sides of the pool, wsol & st_sol accounts "
    );
    let pool_wsol_account = Keypair::new();
    println!(
        "liquidity pool wsol account: {}",
        pool_wsol_account.pubkey()
    );
    let pool_st_sol_account = Keypair::new();
    println!(
        "liquidity pool st_sol account: {}",
        pool_st_sol_account.pubkey()
    );

    let mut transaction = Transaction::new_with_payer(
        &[
            // Account for the stake pool mint
            system_instruction::create_account(
                &config.fee_payer.pubkey(),
                &st_sol_mint_account.pubkey(),
                mint_account_balance,
                TokenMint::LEN as u64,
                &spl_token::id(),
            ),
            // One side of the liq-pool, the wSOL side - TokenAccount::LEN as u64, native empty account to initialize 
             system_instruction::create_account(
                 &config.fee_payer.pubkey(),
                 &pool_wsol_account.pubkey(),
                 pool_fee_account_balance,
                 TokenAccount::LEN as u64,
                 &spl_token::id(),
             ),
            // the other  - TokenAccount::LEN as u64, native empty account to initialize 
            system_instruction::create_account(
                &config.fee_payer.pubkey(),
                &pool_st_sol_account.pubkey(),
                pool_fee_account_balance,
                TokenAccount::LEN as u64,
                &spl_token::id(),
            ),

            // Account for the liq pool wSOL
            system_instruction::create_account(
                &config.fee_payer.pubkey(),
                &liq_pool_account.pubkey(),
                pool_account_balance,
                StakePool::LEN as u64,
                &spl_stake_pool::id(),
            ),

            // Validator stake account list storage
            // system_instruction::create_account(
                // &config.fee_payer.pubkey(),
                // &validator_stake_list.pubkey(),
                // validator_stake_list_balance,
                // ValidatorStakeList::LEN as u64,
                // &spl_stake_pool::id(),
            // ),

            // Initialize pool token mint account
            initialize_mint(
                &spl_token::id(),
                &st_sol_mint_account.pubkey(),
                &config.pda_liq_pool_authority,
                None,
                default_decimals,
            )?,

            // Initialize both sides of the pool, 
            // w-sol side
            initialize_token_account(
                &spl_token::id(),
                &pool_wsol_account.pubkey(),
                &pubkey_from(W_SOL_1111111_MINT_ACCOUNT),
                &config.pda_liq_pool_authority, //"token-owner" is like authorithy, we need a PDA so the program can "sign but not sign" txns
            )?,
            // st-sol side
            initialize_token_account(
                &spl_token::id(),
                &pool_st_sol_account.pubkey(),
                &st_sol_mint_account.pubkey(),
                &config.pda_liq_pool_authority, //"token-owner" is like authorithy, we need a PDA so the program can "sign but not sign" txns
            )?,

            // Initialize stake pool account
            // initialize_pool(
            //     &spl_stake_pool::id(),
            //     &liq_pool_account.pubkey(),
            //     &config.owner.pubkey(),
            //     &validator_stake_list.pubkey(),
            //     &mint_account.pubkey(),
            //     &pool_fee_account.pubkey(),
            //     &spl_token::id(),
            //     PoolInitArgs { fee },
            // )?,
        ],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        total_rent_free_balances + fee_calculator.calculate_fee(&transaction.message()),
    )?;

    let mut signers = vec![
        config.fee_payer.as_ref(),
        &liq_pool_account,
        //&validator_stake_list,
        &st_sol_mint_account,
        &pool_st_sol_account,
        &pool_wsol_account,
        config.owner.as_ref(),
    ];

    unique_signers!(signers);

    transaction.sign(&signers, recent_blockhash);

    Ok(Some(transaction))
}

fn command_vsa_create(config: &Config, pool: &Pubkey, validator: &Pubkey) -> CommandResult {
    let (stake_account, _) =
        PoolProcessor::find_stake_address_for_validator(&spl_stake_pool::id(), &validator, &pool);

    println!("Creating stake account {}", stake_account);

    let mut transaction = Transaction::new_with_payer(
        &[
            // Create new validator stake account address
            create_validator_stake_account(
                &spl_stake_pool::id(),
                &pool,
                &config.fee_payer.pubkey(),
                &stake_account,
                &validator,
                &config.owner.pubkey(),
                &config.owner.pubkey(),
                &solana_program::system_program::id(),
                &stake_program_id(),
            )?,
        ],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    transaction.sign(&[config.fee_payer.as_ref()], recent_blockhash);
    Ok(Some(transaction))
}


fn command_vsa_add(
    config: &Config,
    pool: &Pubkey,
    stake: &Pubkey,
    token_receiver: &Option<Pubkey>,
) -> CommandResult {
    // Get stake pool state
    let pool_data = config.rpc_client.get_account_data(&pool)?;
    let pool_data: StakePool = StakePool::deserialize(pool_data.as_slice()).unwrap();

    let mut total_rent_free_balances: u64 = 0;

    let token_receiver_account = Keypair::new();

    let mut instructions: Vec<Instruction> = vec![];
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];

    // Create token account if not specified
    println!("Create token account if not specified");
    let token_receiver = unwrap_create_token_account(
        &config,
        &token_receiver,
        &token_receiver_account,
        &pool_data.pool_mint,
        &mut instructions,
        |balance| {
            signers.push(&token_receiver_account);
            total_rent_free_balances += balance;
        },
    )?;

    // Calculate Deposit and Withdraw stake pool authorities
    let pool_deposit_authority: Pubkey = PoolProcessor::authority_id(
        &spl_stake_pool::id(),
        pool,
        PoolProcessor::AUTHORITY_DEPOSIT,
        pool_data.deposit_bump_seed,
    )
    .unwrap();
    let pool_withdraw_authority: Pubkey = PoolProcessor::authority_id(
        &spl_stake_pool::id(),
        pool,
        PoolProcessor::AUTHORITY_WITHDRAW,
        pool_data.withdraw_bump_seed,
    )
    .unwrap();

    instructions.extend(vec![
        // Set Withdrawer on stake account to Deposit authority of the stake pool
        authorize_stake(
            &stake,
            &config.owner.pubkey(),
            &pool_deposit_authority,
            StakeAuthorize::Withdrawer,
        ),
        // Set Staker on stake account to Deposit authority of the stake pool
        authorize_stake(
            &stake,
            &config.owner.pubkey(),
            &pool_deposit_authority,
            StakeAuthorize::Staker,
        ),
        // Add validator stake account to the pool
        add_validator_stake_account(
            &spl_stake_pool::id(),
            &pool,
            &config.owner.pubkey(),
            &pool_deposit_authority,
            &pool_withdraw_authority,
            &pool_data.validator_stake_list,
            &stake,
            &token_receiver,
            &pool_data.pool_mint,
            &spl_token::id(),
            &stake_program_id(),
        )?,
    ]);

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        total_rent_free_balances + fee_calculator.calculate_fee(&transaction.message()),
    )?;
    unique_signers!(signers);
    println!("--send txn");
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_vsa_remove(
    config: &Config,
    pool: &Pubkey,
    stake: &Pubkey,
    burn_from: &Pubkey,
    new_authority: &Option<Pubkey>,
) -> CommandResult {
    // Get stake pool state
    let pool_data = config.rpc_client.get_account_data(&pool)?;
    let pool_data: StakePool = StakePool::deserialize(pool_data.as_slice()).unwrap();

    let pool_withdraw_authority: Pubkey = PoolProcessor::authority_id(
        &spl_stake_pool::id(),
        pool,
        PoolProcessor::AUTHORITY_WITHDRAW,
        pool_data.withdraw_bump_seed,
    )
    .unwrap();

    let owner_pubkey = config.owner.pubkey();
    let new_authority = new_authority.as_ref().unwrap_or(&owner_pubkey);

    // Calculate amount of tokens to burn
    let stake_account = config.rpc_client.get_account(&stake)?;
    let tokens_to_burn = stake_amount_to_pool_tokens(&pool_data, stake_account.lamports);

    // Check balance and mint
    let account_data = config.rpc_client.get_account_data(&burn_from)?;
    let account_data: TokenAccount =
        TokenAccount::unpack_from_slice(account_data.as_slice()).unwrap();

    if account_data.mint != pool_data.pool_mint {
        return Err("Wrong token account.".into());
    }

    if account_data.amount < tokens_to_burn {
        return Err(format!(
            "Not enough balance to burn to remove validator stake account from the pool. {} pool tokens needed.",
            lamports_to_sol(tokens_to_burn)
        ).into());
    }

    let mut transaction = Transaction::new_with_payer(
        &[
            // Approve spending token
            approve_token(
                &spl_token::id(),
                &burn_from,
                &pool_withdraw_authority,
                &config.owner.pubkey(),
                &[],
                tokens_to_burn,
            )?,
            // Create new validator stake account address
            remove_validator_stake_account(
                &spl_stake_pool::id(),
                &pool,
                &config.owner.pubkey(),
                &pool_withdraw_authority,
                &new_authority,
                &pool_data.validator_stake_list,
                &stake,
                &burn_from,
                &pool_data.pool_mint,
                &spl_token::id(),
                &stake_program_id(),
            )?,
        ],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    transaction.sign(
        &[config.fee_payer.as_ref(), config.owner.as_ref()],
        recent_blockhash,
    );
    Ok(Some(transaction))
}

fn unwrap_create_token_account<F>(
    config: &Config,
    token_optional: &Option<Pubkey>,
    keypair: &Keypair,
    mint: &Pubkey,
    out_instructions: &mut Vec<Instruction>,
    handler: F,
) -> Result<Pubkey, Error>
where
    F: FnOnce(u64),
{
    let result = match token_optional {
        Some(value) => *value,
        None => {
            // Account for tokens not specified, creating one
            println!("Creating account to receive tokens {}", keypair.pubkey());

            let min_account_balance = config
                .rpc_client
                .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)?;

            out_instructions.extend(vec![
                // Creating new account
                system_instruction::create_account(
                    &config.fee_payer.pubkey(),
                    &keypair.pubkey(),
                    min_account_balance,
                    TokenAccount::LEN as u64,
                    &spl_token::id(),
                ),
                // Initialize token receiver account
                initialize_token_account(
                    &spl_token::id(),
                    &keypair.pubkey(),
                    mint,
                    &config.owner.pubkey(),
                )?,
            ]);

            handler(min_account_balance);

            keypair.pubkey()
        }
    };
    Ok(result)
}

fn command_deposit(
    config: &Config,
    pool: &Pubkey,
    stake: &Pubkey,
    token_receiver: &Option<Pubkey>,
) -> CommandResult {
    // Get stake pool state
    let pool_data = config.rpc_client.get_account_data(&pool)?;
    let pool_data: StakePool = StakePool::deserialize(pool_data.as_slice()).unwrap();

    // Get stake account data
    let stake_data = config.rpc_client.get_account_data(&stake)?;
    let stake_data: StakeState =
        deserialize(stake_data.as_slice()).or(Err("Invalid stake account data"))?;
    let validator: Pubkey = match stake_data {
        StakeState::Stake(_, stake) => Ok(stake.delegation.voter_pubkey),
        _ => Err("Wrong stake account state, must be delegated to validator"),
    }?;

    // Check if this validator has staking account in the pool
    let validator_stake_list_data = config
        .rpc_client
        .get_account_data(&pool_data.validator_stake_list)?;
    let validator_stake_list_data =
        ValidatorStakeList::deserialize(&validator_stake_list_data.as_slice())?;
    if !validator_stake_list_data.contains(&validator) {
        return Err("Stake account for this validator does not exist in the pool.".into());
    }

    // Calculate validator stake account address linked to the pool
    let (validator_stake_account, _) =
        PoolProcessor::find_stake_address_for_validator(&spl_stake_pool::id(), &validator, pool);

    let mut instructions: Vec<Instruction> = vec![];
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];

    let mut total_rent_free_balances: u64 = 0;

    let token_receiver_account = Keypair::new();

    // Create token account if not specified
    let token_receiver = unwrap_create_token_account(
        &config,
        &token_receiver,
        &token_receiver_account,
        &pool_data.pool_mint,
        &mut instructions,
        |balance| {
            signers.push(&token_receiver_account);
            total_rent_free_balances += balance;
        },
    )?;

    // Calculate Deposit and Withdraw stake pool authorities
    let pool_deposit_authority: Pubkey = PoolProcessor::authority_id(
        &spl_stake_pool::id(),
        pool,
        PoolProcessor::AUTHORITY_DEPOSIT,
        pool_data.deposit_bump_seed,
    )
    .unwrap();

    let liq_pool_withdraw_authority: Pubkey = PoolProcessor::authority_id(
        &config.st_sol_mint_account,
        pool,
        PoolProcessor::AUTHORITY_WITHDRAW,
        pool_data.withdraw_bump_seed,
    )
    .unwrap();

    instructions.extend(vec![
        // Set Withdrawer on stake account to Deposit authority of the stake pool
        authorize_stake(
            &stake,
            &config.owner.pubkey(),
            &pool_deposit_authority,
            StakeAuthorize::Withdrawer,
        ),
        // Set Staker on stake account to Deposit authority of the stake pool
        authorize_stake(
            &stake,
            &config.owner.pubkey(),
            &pool_deposit_authority,
            StakeAuthorize::Staker,
        ),
        // Add stake account to the pool
        deposit(
            &spl_stake_pool::id(),
            &pool,
            &pool_data.validator_stake_list,
            &pool_deposit_authority,
            &liq_pool_withdraw_authority,
            &stake,
            &validator_stake_account,
            &token_receiver,
            &pool_data.owner_fee_account,
            &pool_data.pool_mint,
            &spl_token::id(),
            &stake_program_id(),
        )?,
    ]);

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        total_rent_free_balances + fee_calculator.calculate_fee(&transaction.message()),
    )?;
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}


//-------------------------------------
fn command_add_liquidity(
    config: &Config,
    w_sol_source_account: &Pubkey,
    amount: u64,
    token_receiver: &Option<Pubkey>,

) -> CommandResult {

    // Get stake pool state
    println!("&config.stake_pool_state_account {}",&config.stake_pool_state_account);
    // let meta_pool_data_raw = config.rpc_client.get_account_data(&config.stake_pool_state_account)?;
    // let meta_pool_data: StakePool = StakePool::deserialize(meta_pool_data_raw.as_slice()).unwrap();

    let mut instructions: Vec<Instruction> = vec![];
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];

    let mut total_rent_free_balances: u64 = 0;

    let token_receiver_account = Keypair::new();
    // Create token account if not specified
    let token_receiver = unwrap_create_token_account(
        &config,
        &token_receiver,
        &token_receiver_account,
        &config.meta_lp_mint_account,
        &mut instructions,
        |balance| {
            signers.push(&token_receiver_account);
            total_rent_free_balances += balance;
        },
    )?;

    instructions.extend(vec![
        // add liq
        instruction_add_liquidity(
            amount,
            &config.smart_pool_program_id,
            &config.liq_pool_state_account,
            &spl_token::id(),
            &config.meta_lp_mint_account,
            &config.pda_liq_pool_authority,
            &w_sol_source_account,
            &config.fee_payer.pubkey(),
            &config.liq_pool_wsol_account,
            &token_receiver
        ).unwrap()
    ]);

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        total_rent_free_balances + fee_calculator.calculate_fee(&transaction.message()),
    )?;

    unique_signers!(signers);

    transaction.sign(&signers, recent_blockhash);

    Ok(Some(transaction))
}

//-------------------------------------
fn command_sell_st_sol(
    config: &Config,
    user_st_sol_source_account: &Pubkey,
    amount: u64,
    token_receiver: &Option<Pubkey>,

) -> CommandResult {

    // Get stake pool state
    println!("&config.stake_pool_state_account {}",&config.stake_pool_state_account);
    // let meta_pool_data_raw = config.rpc_client.get_account_data(&config.stake_pool_state_account)?;
    // let meta_pool_data: StakePool = StakePool::deserialize(meta_pool_data_raw.as_slice()).unwrap();

    let mut instructions: Vec<Instruction> = vec![];
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];

    let mut total_rent_free_balances: u64 = 0;

    let token_receiver_account = Keypair::new();
    // Create token account if not specified
    let token_receiver = unwrap_create_token_account(
        &config,
        &token_receiver,
        &token_receiver_account,
        &pubkey_from(W_SOL_1111111_MINT_ACCOUNT),
        &mut instructions,
        |balance| {
            signers.push(&token_receiver_account);
            total_rent_free_balances += balance;
        },
    )?;

    instructions.extend(vec![
        // add liq
        instruction_sell_stsol(
            amount,
            &config.smart_pool_program_id,
            &config.stake_pool_state_account,
            &config.liq_pool_state_account,
            &spl_token::id(),
            &config.liq_pool_wsol_account,
            &config.liq_pool_stsol_account,
            &config.pda_liq_pool_authority,
            &token_receiver,
            &user_st_sol_source_account,
            &config.fee_payer.pubkey(),
        ).unwrap()
    ]);

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        total_rent_free_balances + fee_calculator.calculate_fee(&transaction.message()),
    )?;

    unique_signers!(signers);

    transaction.sign(&signers, recent_blockhash);

    Ok(Some(transaction))
}

fn command_list(config: &Config, stake_pool_state: &Pubkey) -> CommandResult {

    println!("Program {:?}",&spl_stake_pool::id());
    println!("State {:?}",&stake_pool_state);
    // Get stake pool state
    let stake_pool_state_acc = config.rpc_client.get_account_data(&stake_pool_state)?;
    let stake_pool_data: StakePool = StakePool::deserialize(stake_pool_state_acc.as_slice()).unwrap();
    println!("{:?}",stake_pool_data);


    let validator_stake_list_acc = config
        .rpc_client
        .get_account_data(&stake_pool_data.validator_stake_list)?;
    let validator_stake_list_data =
        ValidatorStakeList::deserialize(&validator_stake_list_acc.as_slice())?;
    println!("Validators ({})",validator_stake_list_data.validators.len());
    println!("----------------");
    for info in validator_stake_list_data.validators {
        println!("Validator {}\tBalance:{}", info.validator_account, info.balance);
    }
    
        
    let pool_withdraw_authority: Pubkey = PoolProcessor::authority_id(
        &config.smart_pool_program_id,
        stake_pool_state,
        PoolProcessor::AUTHORITY_WITHDRAW,
        stake_pool_data.withdraw_bump_seed,
    )
    .unwrap();

    let accounts = get_authority_accounts(config, &pool_withdraw_authority);

    if accounts.is_empty() {
        return Err("No get_authority_accounts found.".to_string().into());
    }

    let mut total_balance: u64 = 0;
    for (pubkey, account) in accounts {
        let balance = account.lamports;
        total_balance += balance;
        println!("{}\t{} SOL", pubkey, lamports_to_sol(balance));
    }
    println!("Total: {} SOL", lamports_to_sol(total_balance));

    Ok(None)
}

fn command_update(config: &Config, pool: &Pubkey) -> CommandResult {
    // Get stake pool state
    let pool_data = config.rpc_client.get_account_data(&pool)?;
    let pool_data: StakePool = StakePool::deserialize(pool_data.as_slice()).unwrap();
    let validator_stake_list_data = config
        .rpc_client
        .get_account_data(&pool_data.validator_stake_list)?;
    let validator_stake_list_data =
        ValidatorStakeList::deserialize(&validator_stake_list_data.as_slice())?;

    let epoch_info = config.rpc_client.get_epoch_info()?;

    println!("Validators({})",&validator_stake_list_data.validators.len());
    for info in &validator_stake_list_data.validators {
        println!("Validator {}\tBalance:{}", info.validator_account, info.balance);
    }

    let accounts_to_update: Vec<&Pubkey> = validator_stake_list_data
        .validators
        .iter()
        .filter_map(|item| {
            if item.last_update_epoch >= epoch_info.epoch {
                None
            } else {
                Some(&item.validator_account)
            }
        })
        .collect();

    let mut instructions: Vec<Instruction> = vec![];

    for chunk in accounts_to_update.chunks(MAX_ACCOUNTS_TO_UPDATE) {
        instructions.push(update_list_balance(
            &spl_stake_pool::id(),
            &pool_data.validator_stake_list,
            &chunk,
        )?);
    }

    // LMT: If there are no validators registered, we need to call update_pool_balance ANYWAY to update stake_pool.last_update_epoch = clock.epoch
    // in case the pool was create in the prev-epoch and no validators were added in that epoch
    if instructions.is_empty() && validator_stake_list_data.validators.len()>0 {
        println!("Stake pool balances are up to date, no update required.");
        Ok(None)
    } else {
        instructions.push(update_pool_balance(
            &spl_stake_pool::id(),
            pool,
            &pool_data.validator_stake_list,
        )?);

        let mut transaction =
            Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

        let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
        check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
        transaction.sign(&[config.fee_payer.as_ref()], recent_blockhash);
        Ok(Some(transaction))
    }
}

fn stake_amount_to_pool_tokens(pool_data: &StakePool, amount: u64) -> u64 {
    (amount as u128)
        .checked_mul(pool_data.pool_total as u128)
        .unwrap()
        .checked_div(pool_data.stake_total as u128)
        .unwrap() as u64
}

fn pool_tokens_to_stake_amount(pool_data: &StakePool, tokens: u64) -> u64 {
    (tokens as u128)
        .checked_mul(pool_data.stake_total as u128)
        .unwrap()
        .checked_div(pool_data.pool_total as u128)
        .unwrap() as u64
}

#[derive(PartialEq, Debug)]
struct WithdrawAccount {
    pubkey: Pubkey,
    account: Account,
    amount: u64,
}

fn prepare_withdraw_accounts(
    config: &Config,
    pool_withdraw_authority: &Pubkey,
    amount: u64,
) -> Result<Vec<WithdrawAccount>, Error> {
    let mut accounts = get_authority_accounts(config, &pool_withdraw_authority);
    if accounts.is_empty() {
        return Err("No accounts found.".to_string().into());
    }
    let min_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(STAKE_STATE_LEN)?
        + 1;
    pick_withdraw_accounts(&mut accounts, amount, min_balance)
}

fn pick_withdraw_accounts(
    accounts: &mut Vec<(Pubkey, Account)>,
    amount: u64,
    min_balance: u64,
) -> Result<Vec<WithdrawAccount>, Error> {
    // Sort from highest to lowest balance
    accounts.sort_by(|a, b| b.1.lamports.cmp(&a.1.lamports));

    // Prepare the list of accounts to withdraw from
    let mut withdraw_from: Vec<WithdrawAccount> = vec![];
    let mut remaining_amount = amount;

    // Go through available accounts and withdraw from largest to smallest
    for (pubkey, account) in accounts {
        if account.lamports <= min_balance {
            continue;
        }
        let available_for_withdrawal = account.lamports - *MIN_STAKE_BALANCE;
        let withdraw_amount = u64::min(available_for_withdrawal, remaining_amount);

        // Those accounts will be withdrawn completely with `claim` instruction
        withdraw_from.push(WithdrawAccount {
            pubkey: *pubkey,
            account: account.clone(),
            amount: withdraw_amount,
        });
        remaining_amount -= withdraw_amount;

        if remaining_amount == 0 {
            break;
        }
    }

    // Not enough stake to withdraw the specified amount
    if remaining_amount > 0 {
        return Err(format!(
            "No stake accounts found in this pool with enough balance to withdraw {} SOL.",
            lamports_to_sol(amount)
        )
        .into());
    }

    Ok(withdraw_from)
}

fn command_withdraw(
    config: &Config,
    pool: &Pubkey,
    amount: u64,
    burn_from: &Pubkey,
    stake_receiver_param: &Option<Pubkey>,
) -> CommandResult {
    // Get stake pool state
    let pool_data = config.rpc_client.get_account_data(&pool)?;
    let pool_data: StakePool = StakePool::deserialize(pool_data.as_slice()).unwrap();

    let pool_withdraw_authority: Pubkey = PoolProcessor::authority_id(
        &spl_stake_pool::id(),
        pool,
        PoolProcessor::AUTHORITY_WITHDRAW,
        pool_data.withdraw_bump_seed,
    )
    .unwrap();

    // Check burn_from account type
    let account_data = config.rpc_client.get_account_data(&burn_from)?;
    let account_data: TokenAccount =
        TokenAccount::unpack_from_slice(account_data.as_slice()).unwrap();

    if account_data.mint != pool_data.pool_mint {
        return Err("Wrong token account.".into());
    }

    // Check burn_from balance
    if account_data.amount < amount {
        return Err(format!(
            "Not enough token balance to withdraw {} pool tokens.\nMaximum withdraw amount is {} pool tokens.",
            lamports_to_sol(amount),
            lamports_to_sol(account_data.amount)
        )
        .into());
    }

    // Convert pool tokens amount to lamports
    let sol_withdraw_amount = pool_tokens_to_stake_amount(&pool_data, amount);

    // Get the list of accounts to withdraw from
    let withdraw_from: Vec<WithdrawAccount> =
        prepare_withdraw_accounts(config, &pool_withdraw_authority, sol_withdraw_amount)?;

    // Construct transaction to withdraw from withdraw_from account list
    let mut instructions: Vec<Instruction> = vec![];
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    let stake_receiver_account = Keypair::new(); // Will be added to signers if creating new account

    let mut total_rent_free_balances: u64 = 0;

    instructions.push(
        // Approve spending token
        approve_token(
            &spl_token::id(),
            &burn_from,
            &pool_withdraw_authority,
            &config.owner.pubkey(),
            &[],
            amount,
        )?,
    );

    // Use separate mutable variable because withdraw might create a new account
    let mut stake_receiver: Option<Pubkey> = *stake_receiver_param;

    // Go through prepared accounts and withdraw/claim them
    for withdraw_stake in withdraw_from {
        println!(
            "Withdrawing from account {}, amount {} SOL",
            withdraw_stake.pubkey,
            lamports_to_sol(withdraw_stake.amount)
        );

        if stake_receiver.is_none() {
            // Account for tokens not specified, creating one
            println!(
                "Creating account to receive stake {}",
                stake_receiver_account.pubkey()
            );

            let stake_receiver_account_balance = config
                .rpc_client
                .get_minimum_balance_for_rent_exemption(STAKE_STATE_LEN)?;

            instructions.push(
                // Creating new account
                system_instruction::create_account(
                    &config.fee_payer.pubkey(),
                    &stake_receiver_account.pubkey(),
                    stake_receiver_account_balance,
                    STAKE_STATE_LEN as u64,
                    &stake_program_id(),
                ),
            );

            signers.push(&stake_receiver_account);

            total_rent_free_balances += stake_receiver_account_balance;

            stake_receiver = Some(stake_receiver_account.pubkey());
        }

        instructions.push(withdraw(
            &spl_stake_pool::id(),
            &pool,
            &pool_data.validator_stake_list,
            &pool_withdraw_authority,
            &withdraw_stake.pubkey,
            &stake_receiver.unwrap(), // Cannot be none at this point
            &config.owner.pubkey(),
            &burn_from,
            &pool_data.pool_mint,
            &spl_token::id(),
            &stake_program_id(),
            withdraw_stake.amount,
        )?);
    }

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        total_rent_free_balances + fee_calculator.calculate_fee(&transaction.message()),
    )?;
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);

    Ok(Some(transaction))
}

fn command_set_staking_auth(
    config: &Config,
    pool: &Pubkey,
    stake_account: &Pubkey,
    new_staker: &Pubkey,
) -> CommandResult {
    let pool_data = config.rpc_client.get_account_data(&pool)?;
    let pool_data: StakePool = StakePool::deserialize(pool_data.as_slice()).unwrap();

    let pool_withdraw_authority: Pubkey = PoolProcessor::authority_id(
        &spl_stake_pool::id(),
        pool,
        PoolProcessor::AUTHORITY_WITHDRAW,
        pool_data.withdraw_bump_seed,
    )
    .unwrap();

    let mut transaction = Transaction::new_with_payer(
        &[set_staking_authority(
            &spl_stake_pool::id(),
            &pool,
            &config.owner.pubkey(),
            &pool_withdraw_authority,
            &stake_account,
            &new_staker,
            &stake_program_id(),
        )?],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_set_owner(
    config: &Config,
    pool: &Pubkey,
    new_owner: &Option<Pubkey>,
    new_fee_receiver: &Option<Pubkey>,
) -> CommandResult {
    let pool_data = config.rpc_client.get_account_data(&pool)?;
    let pool_data: StakePool = StakePool::deserialize(pool_data.as_slice()).unwrap();

    // If new accounts are missing in the arguments use the old ones
    let new_owner: Pubkey = match new_owner {
        None => pool_data.owner,
        Some(value) => *value,
    };
    let new_fee_receiver: Pubkey = match new_fee_receiver {
        None => pool_data.owner_fee_account,
        Some(value) => {
            // Check for fee receiver being a valid token account and have to same mint as the stake pool
            let account_data = config.rpc_client.get_account_data(value)?;
            let account_data: TokenAccount =
                match TokenAccount::unpack_from_slice(account_data.as_slice()) {
                    Ok(data) => data,
                    Err(_) => {
                        return Err(format!("{} is not a token account", value).into());
                    }
                };
            if account_data.mint != pool_data.pool_mint {
                return Err("Fee receiver account belongs to a different mint"
                    .to_string()
                    .into());
            }
            *value
        }
    };

    let mut transaction = Transaction::new_with_payer(
        &[set_owner(
            &spl_stake_pool::id(),
            &pool,
            &config.owner.pubkey(),
            &new_owner,
            &new_fee_receiver,
        )?],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn main() {
    let matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg({
            let arg = Arg::with_name("config_file")
                .short("C")
                .long("config")
                .value_name("PATH")
                .takes_value(true)
                .global(true)
                .help("Configuration file to use");
            if let Some(ref config_file) = *solana_cli_config::CONFIG_FILE {
                arg.default_value(&config_file)
            } else {
                arg
            }
        })
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .takes_value(false)
                .global(true)
                .help("Show additional information"),
        )
        .arg(
            Arg::with_name("json_rpc_url")
                .long("url")
                .value_name("URL")
                .takes_value(true)
                .validator(is_url)
                .help("JSON RPC URL for the cluster.  Default from the configuration file."),
        )
        .arg(
            Arg::with_name("owner")
                .long("owner")
                .value_name("KEYPAIR")
                .validator(is_keypair)
                .takes_value(true)
                .help(
                    "Specify the stake pool or stake account owner. \
                     This may be a keypair file, the ASK keyword. \
                     Defaults to the client keypair.",
                ),
        )
        .arg(
            Arg::with_name("fee_payer")
                .long("fee-payer")
                .value_name("KEYPAIR")
                .validator(is_keypair)
                .takes_value(true)
                .help(
                    "Specify the fee-payer account. \
                     This may be a keypair file, the ASK keyword. \
                     Defaults to the client keypair.",
                ),
        )
        // .subcommand(SubCommand::with_name("create-pool").about("Create a new stake pool")
        //     .arg(
        //         Arg::with_name("fee_numerator")
        //             .long("fee-numerator")
        //             .short("n")
        //             .validator(is_parsable::<u64>)
        //             .value_name("NUMERATOR")
        //             .takes_value(true)
        //             .required(true)
        //             .help("Fee numerator, fee amount is numerator divided by denominator."),
        //     )
        //     .arg(
        //         Arg::with_name("fee_denominator")
        //             .long("fee-denominator")
        //             .short("d")
        //             .validator(is_parsable::<u64>)
        //             .value_name("DENOMINATOR")
        //             .takes_value(true)
        //             .required(true)
        //             .help("Fee denominator, fee amount is numerator divided by denominator."),
        //     )
        // )
        .subcommand(SubCommand::with_name("create-liq-pool").about("Create the liquidity pool & $METALP token")
        )
        .subcommand(SubCommand::with_name("create-validator-stake").about("Create a new validator stake account to use with the pool")
            //    
            // LMT commented, pool is fixed in this cli
            // .arg(
            //     Arg::with_name("pool")
            //         .long("pool")
            //         .validator(is_pubkey)
            //         .value_name("ADDRESS")
            //         .takes_value(true)
            //         .required(true)
            //         .help("Stake pool address"),
            // )
            .arg(
                Arg::with_name("validator")
                    .long("validator")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Validator this stake account will vote for"),
            )
        )
        .subcommand(SubCommand::with_name("add-validator-stake").about("Add validator stake account to the stake pool. Must be signed by the pool owner.")
            // .arg(
            //     Arg::with_name("pool")
            //         .long("pool")
            //         .validator(is_pubkey)
            //         .value_name("ADDRESS")
            //         .takes_value(true)
            //         .required(true)
            //         .help("Stake pool address"),
            // )
            .arg(
                Arg::with_name("stake")
                    .long("stake")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Stake account to add to the pool"),
            )
            .arg(
                Arg::with_name("token_receiver")
                    .long("token-receiver")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Account to receive pool token. Must be initialized account of the stake pool token. Defaults to the new pool token account."),
            )
        )
        .subcommand(SubCommand::with_name("remove-validator-stake").about("Add validator stake account to the stake pool. Must be signed by the pool owner.")
            // .arg(
            //     Arg::with_name("pool")
            //         .long("pool")
            //         .validator(is_pubkey)
            //         .value_name("ADDRESS")
            //         .takes_value(true)
            //         .required(true)
            //         .help("Stake pool address"),
            // )
            .arg(
                Arg::with_name("stake")
                    .long("stake")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Stake account to remove from the pool"),
            )
            .arg(
                Arg::with_name("burn_from")
                    .long("burn-from")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Token account to burn pool token from. Must have enough tokens to burn for the full stake address balance."),
            )
            .arg(
                Arg::with_name("new_authority")
                    .long("new-authority")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("New authority to set as Staker and Withdrawer in the stake account removed from the pool. Defaults to the wallet owner pubkey."),
            )
        )
        .subcommand(SubCommand::with_name("deposit").about("Add stake account to the stake pool")
            // .arg(
            //     Arg::with_name("pool")
            //         .long("pool")
            //         .validator(is_pubkey)
            //         .value_name("ADDRESS")
            //         .takes_value(true)
            //         .required(true)
            //         .help("Stake pool address"),
            // )
            .arg(
                Arg::with_name("stake")
                    .long("stake")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Stake address to join the pool"),
            )
            .arg(
                Arg::with_name("token_receiver")
                    .long("token-receiver")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Account to receive pool token. Must be initialized account of the stake pool token. Defaults to the new pool token account."),
            )
        )
        .subcommand(SubCommand::with_name("list").about("List stake accounts managed by this pool")
            // .arg(
            //     Arg::with_name("pool")
            //         .long("pool")
            //         .validator(is_pubkey)
            //         .value_name("ADDRESS")
            //         .takes_value(true)
            //         .required(true)
            //         .help("Stake pool address."),
            // )
        )
        .subcommand(SubCommand::with_name("update").about("Updates all balances in the pool after validator stake accounts receive rewards.")
            // .arg(
            //     Arg::with_name("pool")
            //         .long("pool")
            //         .validator(is_pubkey)
            //         .value_name("ADDRESS")
            //         .takes_value(true)
            //         .required(true)
            //         .help("Stake pool address."),
            // )
        )
        .subcommand(SubCommand::with_name("withdraw").about("Withdraw amount from the stake pool")
            // .arg(
            //     Arg::with_name("pool")
            //         .long("pool")
            //         .validator(is_pubkey)
            //         .value_name("ADDRESS")
            //         .takes_value(true)
            //         .required(true)
            //         .help("Stake pool address."),
            // )
            .arg(
                Arg::with_name("amount")
                    .long("amount")
                    .validator(is_amount)
                    .value_name("AMOUNT")
                    .takes_value(true)
                    .required(true)
                    .help("Amount of pool tokens to burn and get rewards."),
            )
            .arg(
                Arg::with_name("burn_from")
                    .long("burn-from")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Account to burn tokens from. Must be owned by the client."),
            )
            .arg(
                Arg::with_name("stake_receiver")
                    .long("stake-receiver")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Stake account to receive SOL from the stake pool. Defaults to a new stake account."),
            )
        )
        .subcommand(SubCommand::with_name("set-staking-auth").about("Changes staking authority of one of the accounts from the stake pool.")
            // .arg(
            //     Arg::with_name("pool")
            //         .long("pool")
            //         .validator(is_pubkey)
            //         .value_name("ADDRESS")
            //         .takes_value(true)
            //         .required(true)
            //         .help("Stake pool address."),
            // )
            .arg(
                Arg::with_name("stake_account")
                    .long("stake-account")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Stake account address to change staking authority."),
            )
            .arg(
                Arg::with_name("new_staker")
                    .long("new-staker")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Public key of the new staker account."),
            )
        )
        .subcommand(SubCommand::with_name("set-owner").about("Changes owner or fee receiver account for the stake pool.")
            // .arg(
            //     Arg::with_name("pool")
            //         .long("pool")
            //         .validator(is_pubkey)
            //         .value_name("ADDRESS")
            //         .takes_value(true)
            //         .required(true)
            //         .help("Stake pool address."),
            // )
            .arg(
                Arg::with_name("new_owner")
                    .long("new-owner")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Public key for the new stake pool owner."),
            )
            .arg(
                Arg::with_name("new_fee_receiver")
                    .long("new-fee-receiver")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Public key for the new account to set as the stake pool fee receiver."),
            )
            .group(ArgGroup::with_name("new_accounts")
                .arg("new_owner")
                .arg("new_fee_receiver")
                .required(true)
                .multiple(true)
            )
        )
        .subcommand(SubCommand::with_name("add-liquidity").about("Add wSOL amount to wSOL/stSOL Liquidity pool")
            .arg(
                Arg::with_name("amount")
                    .short("a")
                    .index(1)
                    .validator(is_amount)
                    .value_name("AMOUNT")
                    .takes_value(true)
                    .required(true)
                    .help("Amount of wSOL to add."),
            )
            .arg(
                Arg::with_name("source")
                    .short("s")
                    .long("source")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("wSOL token account to take wSOL from. Must be owned by the signer."),
            )
        )
        .subcommand(SubCommand::with_name("sell").about("Sell stSOL for wSOL using the liquidity pool")
            .arg(
                Arg::with_name("amount")
                    .short("a")
                    .index(1)
                    .validator(is_amount)
                    .value_name("AMOUNT")
                    .takes_value(true)
                    .required(true)
                    .help("Amount of stSOL to sell."),
            )
            .arg(
                Arg::with_name("source")
                    .short("s")
                    .long("source")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("stSOL token account to take stSOL from. Must be owned by the signer."),
            )
        )
        .get_matches();

    let mut wallet_manager = None;
    let config = {
        let cli_config = if let Some(config_file) = matches.value_of("config_file") {
            solana_cli_config::Config::load(config_file).unwrap_or_default()
        } else {
            solana_cli_config::Config::default()
        };
        let json_rpc_url = value_t!(matches, "json_rpc_url", String)
            .unwrap_or_else(|_| cli_config.json_rpc_url.clone());

        let owner = signer_from_path(
            &matches,
            &cli_config.keypair_path,
            "owner",
            &mut wallet_manager,
        )
        .unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });
        let fee_payer = signer_from_path(
            &matches,
            &cli_config.keypair_path,
            "fee_payer",
            &mut wallet_manager,
        )
        .unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });
        let verbose = matches.is_present("verbose");

        let smart_pool_program_id = pubkey_from(SMART_POOL_PROGRAM_ACCOUNT_ID);
        let liq_pool_state_account = pubkey_from(LIQ_POOL_ACCOUNT);

        // Calculate PDA withdraw authority used for minting pool tokens
        let (pda_liq_pool_authority, _) = PoolProcessor::find_authority_bump_seed(
            &smart_pool_program_id,
            &liq_pool_state_account,
            PoolProcessor::AUTHORITY,
        );

        Config {
            rpc_client: RpcClient::new(json_rpc_url),
            verbose,
            owner,
            fee_payer,
            commitment_config: CommitmentConfig::confirmed(),

            smart_pool_program_id,
            stake_pool_state_account: pubkey_from(STAKE_POOL_STATE_ACCOUNT),

            liq_pool_state_account,
            pda_liq_pool_authority,
            meta_lp_mint_account:pubkey_from(META_LP_MINT_ACCOUNT),
            st_sol_mint_account: pubkey_from(ST_SOL_MINT_ACCOUNT),
            liq_pool_wsol_account:pubkey_from(LIQ_POOL_WSOL_ACCOUNT),
            liq_pool_stsol_account:pubkey_from(LIQ_POOL_ST_SOL_ACCOUNT),
            
        }
    };

    solana_logger::setup_with_default("solana=info");

    let pool_account = config.stake_pool_state_account;

    let _ = match matches.subcommand() {
        // ("create-pool", Some(arg_matches)) => {
        //     let numerator = value_t_or_exit!(arg_matches, "fee_numerator", u64);
        //     let denominator = value_t_or_exit!(arg_matches, "fee_denominator", u64);
        //     command_create_pool(
        //         &config,
        //         PoolFee {
        //             numerator,
        //             denominator,
        //         },
        //     )
        // }
        ("create-liq-pool", Some(_arg_matches)) => {
            command_create_liquidity_pool(
                &config
            )
        }
        ("create-validator-stake", Some(arg_matches)) => {
            //let pool_account: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            let validator_account: Pubkey = pubkey_of(arg_matches, "validator").unwrap();
            command_vsa_create(&config, &pool_account, &validator_account)
        }
        ("add-validator-stake", Some(arg_matches)) => {
            //let pool_account: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            
            let stake_account: Pubkey = pubkey_of(arg_matches, "stake").unwrap();
            let token_receiver: Option<Pubkey> = pubkey_of(arg_matches, "token_receiver");
            command_vsa_add(&config, &pool_account, &stake_account, &token_receiver)
        }
        ("remove-validator-stake", Some(arg_matches)) => {
            //let pool_account: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            
            let stake_account: Pubkey = pubkey_of(arg_matches, "stake").unwrap();
            let burn_from: Pubkey = pubkey_of(arg_matches, "burn_from").unwrap();
            let new_authority: Option<Pubkey> = pubkey_of(arg_matches, "new_authority");
            command_vsa_remove(
                &config,
                &pool_account,
                &stake_account,
                &burn_from,
                &new_authority,
            )
        }
        ("deposit", Some(arg_matches)) => {
            
            //let pool_account: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            let stake_account: Pubkey = pubkey_of(arg_matches, "stake").unwrap();
            let token_receiver: Option<Pubkey> = pubkey_of(arg_matches, "token_receiver");
            command_deposit(&config, &pool_account, &stake_account, &token_receiver)
        }
        ("list", Some(_arg_matches)) => {
            //let pool_account: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            command_list(&config, &pool_account)
        }
        ("update", Some(_arg_matches)) => {
            //let pool_account: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            
            command_update(&config, &pool_account)
        }
        ("withdraw", Some(arg_matches)) => {
            //let pool_account: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            
            let burn_from: Pubkey = pubkey_of(arg_matches, "burn_from").unwrap();
            // convert from float to int, using sol_to_lamports because they have the same precision as SOL
            let amount: u64 = sol_to_lamports(value_t_or_exit!(arg_matches, "amount", f64));
            let stake_receiver: Option<Pubkey> = pubkey_of(arg_matches, "stake_receiver");
            command_withdraw(&config, &pool_account, amount, &burn_from, &stake_receiver)
        }

        ("add-liquidity", Some(arg_matches)) => {
            //let pool_account: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            let source: Pubkey = pubkey_of(arg_matches, "source").unwrap();
            // convert from float to int, using sol_to_lamports because they have the same precision as SOL
            let amount: u64 = sol_to_lamports(value_t_or_exit!(arg_matches, "amount", f64));
            command_add_liquidity(&config, &source, amount, &None)
        }
        
        ("sell", Some(arg_matches)) => {
            // convert from float to int, using sol_to_lamports because they have the same precision as SOL
            let amount: u64 = sol_to_lamports(value_t_or_exit!(arg_matches, "amount", f64));
            // source token acc with stSOL
            let source: Pubkey = pubkey_of(arg_matches, "source").unwrap();
            command_sell_st_sol(&config, &source, amount, &None)
        }

        ("set-staking-auth", Some(arg_matches)) => {
            //let pool_account: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            
            let stake_account: Pubkey = pubkey_of(arg_matches, "stake_account").unwrap();
            let new_staker: Pubkey = pubkey_of(arg_matches, "new_staker").unwrap();
            command_set_staking_auth(&config, &pool_account, &stake_account, &new_staker)
        }
        ("set-owner", Some(arg_matches)) => {
            //let pool_account: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            
            let new_owner: Option<Pubkey> = pubkey_of(arg_matches, "new_owner");
            let new_fee_receiver: Option<Pubkey> = pubkey_of(arg_matches, "new_fee_receiver");
            command_set_owner(&config, &pool_account, &new_owner, &new_fee_receiver)
        }
        _ => unreachable!(),
    }
    .and_then(|transaction| {
        if let Some(transaction) = transaction {
            // TODO: Upgrade to solana-client 1.3 and
            // `send_and_confirm_transaction_with_spinner_and_commitment()` with single
            // confirmation by default for better UX
            let signature = config
                .rpc_client
                .send_and_confirm_transaction_with_spinner_and_commitment(
                    &transaction,
                    config.commitment_config,
                )?;
            println!("Signature: {}", signature);
        }
        Ok(())
    })
    .map_err(|err| {
        eprintln!("{}", err);
        exit(1);
    });
}
