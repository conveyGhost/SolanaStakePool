#![cfg(feature = "test-bpf")]

mod helpers;
use helpers::*;

use solana_program::pubkey::Pubkey;

use solana_program::{hash::Hash};
use solana_program_test::BanksClient;
use solana_sdk::{
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::Transaction,
    transaction::TransactionError,
    transport::TransportError,
};
use spl_stake_pool::*;
use spl_stake_pool::processor::Processor;
use spl_token::error::TokenError;

async fn setup() -> (
    BanksClient,
    Keypair,
    Hash,
    StakePoolAccounts,
    ValidatorStakeAccount,
) {

    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    
    let stake_pool_accounts = 
        StakePoolAccounts::new();
    
    stake_pool_accounts
        .initialize_stake_pool(&mut banks_client, &payer, &recent_blockhash)
        .await
        .unwrap();

    println!("--- about to call simple_add_validator_stake_account");
    let validator_stake_account: ValidatorStakeAccount = simple_add_validator_stake_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &stake_pool_accounts,
    ).await;

    println!("validator_stake_account {:?}",validator_stake_account.stake_account);

    return (
        banks_client,
        payer,
        recent_blockhash,
        stake_pool_accounts,
        validator_stake_account,
    )
}

#[tokio::test]
async fn test_add_liquidity() {
    
    println!("---------------------------------");
    println!("---- START test_add_liquidity ---");
    println!("---------------------------------");

    let (
        mut banks_client,
        payer,
        recent_blockhash,
        stake_pool_accounts,
        validator_stake_account,
    ) = setup().await;

    println!("--- about to call prepare_wsol_deposit");
    let deposit_info: DepositInfo = prepare_wsol_deposit(
        &mut banks_client,
        &payer,
        &recent_blockhash,
    )
    .await;
    println!("wsol_tokens balance={}",deposit_info.pool_tokens);
    let wsol_to_deposit = deposit_info.pool_tokens / 2;

    // Create lp token account to send tokens to the user
    let metal_lp_user_dest = Keypair::new();
    println!("create_token_account metal_lp_user_dest");
    let result = create_token_account (
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &metal_lp_user_dest,
        &stake_pool_accounts.meta_lp_mint_acc.pubkey(),
        &payer.pubkey()
    )
    .await;

    // Save state before addliq
    let prev_liq_pool_wsol_dest_account_balance =
    get_token_balance(&mut banks_client, &stake_pool_accounts.liq_pool_wsol_acc.pubkey()).await;
    println!("prev_liq_pool_wsol_dest_account_balance {}",prev_liq_pool_wsol_dest_account_balance);

    // Save user token balance
    let user_token_balance_before =
        get_token_balance(&mut banks_client, &deposit_info.user_pool_account).await;

    //let new_authority = Pubkey::new_unique();
    //----------------------
    let mut transaction = Transaction::new_with_payer(
        &[instruction::instruction_add_liquidity(
            wsol_to_deposit,
            &id(),
            &stake_pool_accounts.liq_pool_state_acc.pubkey(),
            &spl_token::id(),
            &stake_pool_accounts.meta_lp_mint_acc.pubkey(),
            &stake_pool_accounts.liq_pool_authority,
            &deposit_info.user_pool_account,//  .user_wsol_source_account,
            &payer.pubkey(), //user acc withdraw auth
            &stake_pool_accounts.liq_pool_wsol_acc.pubkey(),
            &metal_lp_user_dest.pubkey(),
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    println!("----------------------------------------");
    println!("-- SEND TXN instruction_add_liquidity --");
    println!("----------------------------------------");
    transaction.sign(&[&payer], recent_blockhash);
    let result = banks_client.process_transaction(transaction)
        .await;
    //println!("{:?}",result);

    assert!(!result.is_err(), "TXN ERROR");

    // result.err()
    // .unwrap();

    // Check liq-pool stats
    let liq_pool_wsol_dest_account_balance =
    get_token_balance(&mut banks_client, &stake_pool_accounts.liq_pool_wsol_acc.pubkey()).await;
    println!("liq_pool_wsol_dest_account_balance {}",liq_pool_wsol_dest_account_balance);
    assert_eq!(
        liq_pool_wsol_dest_account_balance,
        prev_liq_pool_wsol_dest_account_balance + wsol_to_deposit
    );

    // Check tokens deposited
    let user_token_balance =
        get_token_balance(&mut banks_client, &deposit_info.user_pool_account).await;
    assert_eq!(
        user_token_balance,
        user_token_balance_before - wsol_to_deposit
    );

    // Check meta-lp tokens received
    // {
    //     let user_token_balance =
    //         get_token_balance(&mut banks_client, &metal_lp_user_dest.pubkey()).await;
    //     assert_eq!(
    //         user_token_balance,
    //         wsol_to_deposit
    //     );
    // }

    // Check user recipient stake account balance
    // let user_stake_recipient_account =
    //     get_account(&mut banks_client, &user_stake_recipient.pubkey()).await;
    // assert_eq!(
    //     user_stake_recipient_account.lamports,
    //     initial_stake_lamports + wsol_to_deposit
    // );
}

#[tokio::test]
async fn test_sell_st_sol() {
    
    println!("-------------------------------");
    println!("---- START test_sell_st_sol ---");
    println!("-------------------------------");

    let (
        mut banks_client,
        payer,
        recent_blockhash,
        stake_pool_accounts,
        validator_stake_account,
    ) = setup().await;

    // Get stake pool stake (and check if it is initialized)
    let stake_pool_account = get_account(&mut banks_client, &stake_pool_accounts.stake_pool.pubkey()).await;
    let stake_pool_data_before =
        state::StakePool::deserialize(&stake_pool_account.data.as_slice()).unwrap();
    if !stake_pool_data_before.is_initialized() {
        panic!("stake_pool_data_before not initialized");
    }

    println!("--- about to call simple_deposit");
    //call simple_deposit so the user_acc has some stSOL to sell
    // simple_deposit  does the entire thing: creates acc, stakes and deposits, so the user acc gets stsOL
    let deposit_info: DepositInfo = simple_deposit(//banks_client: &mut BanksClient, payer: &Keypair, recent_blockhash: &Hash, 
        //stake_pool_accounts: &StakePoolAccounts, validator_stake_account: &ValidatorStakeAccount,
        &mut banks_client,
        &payer, true,
        &recent_blockhash,
        &stake_pool_accounts,
        &validator_stake_account
    ).await;

    // let deposit_info: DepositInfo = prepare_st_sol_deposit(
    //     &mut banks_client,
    //     &stake_pool_accounts.pool_mint.pubkey(),
    //     &stake_pool_data_before.pool_mint_autho
    //     &payer,
    //     &recent_blockhash,
    // )
    // .await;

    let prev_user_stsol_account_balance =
        get_token_balance(&mut banks_client, &deposit_info.user_pool_account).await;
    println!("prev_user_stsol_account_balance={}",prev_user_stsol_account_balance);

    // Create wsol dest account to send wsol
    let wsol_user_dest_acc = Keypair::new();
    println!("create_token_account wsol_user_dest_acc");
    let result = create_token_account (
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &wsol_user_dest_acc,
        &String::from(W_SOL_1111111_MINT_ACCOUNT).parse().unwrap(),
        &payer.pubkey()
    )
    .await;

    // Save state before sell
    let prev_liq_pool_wsol_account_balance = get_token_balance(&mut banks_client, &stake_pool_accounts.liq_pool_wsol_acc.pubkey()).await;
    let prev_liq_pool_st_sol_account_balance = get_token_balance(&mut banks_client, &stake_pool_accounts.liq_pool_st_sol_acc.pubkey()).await;
    println!("--- prev_liq_pool wsol/stSOL {}/{}",prev_liq_pool_wsol_account_balance,prev_liq_pool_st_sol_account_balance);

    let pre_user_stsol_balance = get_token_balance(&mut banks_client, &deposit_info.user_pool_account).await;
    println!("-- pre_user_st_sol_balance {}",pre_user_stsol_balance);

    let stsol_to_sell:u64 = 50_000;

    //let new_authority = Pubkey::new_unique();
    //----------------------
    let mut transaction = Transaction::new_with_payer(
        &[instruction::instruction_sell_stsol(
            stsol_to_sell,
            &id(),
            &stake_pool_accounts.stake_pool.pubkey(),
            &stake_pool_accounts.liq_pool_state_acc.pubkey(),
            &spl_token::id(),
            &stake_pool_accounts.liq_pool_wsol_acc.pubkey(),
            &stake_pool_accounts.liq_pool_st_sol_acc.pubkey(),
            &stake_pool_accounts.liq_pool_authority,
            &wsol_user_dest_acc.pubkey(), //where to send the wsol
            &deposit_info.user_pool_account,//  .user_source_account,
            &payer.pubkey(), //user acc withdraw auth
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    println!("-------------------------------------");
    println!("-- SEND TXN instruction_sell_stsol --");
    println!("-------------------------------------");
    transaction.sign(&[&payer], recent_blockhash);
    let result = banks_client.process_transaction(transaction)
        .await;
    //println!("{:?}",result);

    assert!(!result.is_err(), "TXN ERROR");

    // result.err()
    // .unwrap();

    let valued = stsol_to_sell; //TODO compute value correctly
    let fee = processor::proportional(valued,stake_pool_data_before.fee.numerator as u128, stake_pool_data_before.fee.denominator as u128).unwrap();

    // Check liq-pool wsol balance after sell
    // Check liq-pool st_sol_tokens after sell
    let post_liq_pool_wsol_account_balance = get_token_balance(&mut banks_client, &stake_pool_accounts.liq_pool_wsol_acc.pubkey()).await;
    let post_liq_pool_st_sol_account_balance = get_token_balance(&mut banks_client, &stake_pool_accounts.liq_pool_st_sol_acc.pubkey()).await;
    println!("-- post_liq_pool wsol/stSOL {}/{}",post_liq_pool_wsol_account_balance,post_liq_pool_st_sol_account_balance);
    assert_eq!(
        post_liq_pool_wsol_account_balance,
        prev_liq_pool_wsol_account_balance - stsol_to_sell + fee
    );
    assert_eq!(
        post_liq_pool_st_sol_account_balance,
        prev_liq_pool_st_sol_account_balance + stsol_to_sell
    );

    // Check user stSol balance after sell
    let post_user_stsol_balance =
        get_token_balance(&mut banks_client, &deposit_info.user_pool_account).await;
    println!("-- post_user_st_sol_balance {}",post_user_stsol_balance);
    assert_eq!(
        post_user_stsol_balance,
        prev_user_stsol_account_balance - stsol_to_sell
    );

    // Check user recipient stake account balance
    // let user_stake_recipient_account =
    //     get_account(&mut banks_client, &user_stake_recipient.pubkey()).await;
    // assert_eq!(
    //     user_stake_recipient_account.lamports,
    //     initial_stake_lamports + wsol_to_deposit
    // );
}

/*
#[tokio::test]
async fn test_stake_pool_withdraw_with_wrong_stake_program() {
    let (
        mut banks_client,
        payer,
        recent_blockhash,
        stake_pool_accounts,
        validator_stake_account,
        deposit_info,
        tokens_to_burn,
    ) = setup().await;

    // Create stake account to withdraw to
    let user_stake_recipient = Keypair::new();

    let new_authority = Pubkey::new_unique();
    let wrong_stake_program = Keypair::new();

    let mut transaction = Transaction::new_with_payer(
        &[instruction::withdraw(
            &id(),
            &stake_pool_accounts.stake_pool.pubkey(),
            &stake_pool_accounts.validator_stake_list.pubkey(),
            &stake_pool_accounts.withdraw_authority,
            &validator_stake_account.stake_account,
            &user_stake_recipient.pubkey(),
            &new_authority,
            &deposit_info.user_pool_account,
            &stake_pool_accounts.pool_mint.pubkey(),
            &spl_token::id(),
            &wrong_stake_program.pubkey(),
            tokens_to_burn,
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    let transaction_error = banks_client
        .process_transaction(transaction)
        .await
        .err()
        .unwrap();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(_, error)) => {
            assert_eq!(error, InstructionError::IncorrectProgramId);
        }
        _ => panic!("Wrong error occurs while try to withdraw with wrong stake program ID"),
    }
}

#[tokio::test]
async fn test_stake_pool_withdraw_with_wrong_withdraw_authority() {
    let (
        mut banks_client,
        payer,
        recent_blockhash,
        mut stake_pool_accounts,
        validator_stake_account,
        deposit_info,
        tokens_to_burn,
    ) = setup().await;

    // Create stake account to withdraw to
    let user_stake_recipient = Keypair::new();

    let new_authority = Pubkey::new_unique();
    stake_pool_accounts.withdraw_authority = Keypair::new().pubkey();

    let transaction_error = stake_pool_accounts
        .withdraw_stake(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &user_stake_recipient.pubkey(),
            &deposit_info.user_pool_account,
            &validator_stake_account.stake_account,
            &new_authority,
            tokens_to_burn,
        )
        .await
        .err()
        .unwrap();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(error_index),
        )) => {
            let program_error = error::StakePoolError::InvalidProgramAddress as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!("Wrong error occurs while try to withdraw with wrong withdraw authority"),
    }
}

#[tokio::test]
async fn test_stake_pool_withdraw_with_wrong_token_program_id() {
    let (
        mut banks_client,
        payer,
        recent_blockhash,
        stake_pool_accounts,
        validator_stake_account,
        deposit_info,
        tokens_to_burn,
    ) = setup().await;

    // Create stake account to withdraw to
    let user_stake_recipient = Keypair::new();

    let new_authority = Pubkey::new_unique();
    let wrong_token_program = Keypair::new();

    let mut transaction = Transaction::new_with_payer(
        &[instruction::withdraw(
            &id(),
            &stake_pool_accounts.stake_pool.pubkey(),
            &stake_pool_accounts.validator_stake_list.pubkey(),
            &stake_pool_accounts.withdraw_authority,
            &validator_stake_account.stake_account,
            &user_stake_recipient.pubkey(),
            &new_authority,
            &deposit_info.user_pool_account,
            &stake_pool_accounts.pool_mint.pubkey(),
            &wrong_token_program.pubkey(),
            &stake::id(),
            tokens_to_burn,
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    let transaction_error = banks_client
        .process_transaction(transaction)
        .await
        .err()
        .unwrap();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(_, error)) => {
            assert_eq!(error, InstructionError::IncorrectProgramId);
        }
        _ => panic!("Wrong error occurs while try to withdraw with wrong token program ID"),
    }
}

#[tokio::test]
async fn test_stake_pool_withdraw_with_wrong_validator_stake_list() {
    let (
        mut banks_client,
        payer,
        recent_blockhash,
        mut stake_pool_accounts,
        validator_stake_account,
        deposit_info,
        tokens_to_burn,
    ) = setup().await;

    // Create stake account to withdraw to
    let user_stake_recipient = Keypair::new();

    let new_authority = Pubkey::new_unique();
    stake_pool_accounts.validator_stake_list = Keypair::new();

    let transaction_error = stake_pool_accounts
        .withdraw_stake(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &user_stake_recipient.pubkey(),
            &deposit_info.user_pool_account,
            &validator_stake_account.stake_account,
            &new_authority,
            tokens_to_burn,
        )
        .await
        .err()
        .unwrap();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(error_index),
        )) => {
            let program_error = error::StakePoolError::InvalidValidatorStakeList as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!(
            "Wrong error occurs while try to withdraw with wrong validator stake list account"
        ),
    }
}

#[tokio::test]
async fn test_stake_pool_withdraw_when_stake_acc_not_in_stake_state() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let stake_pool_accounts = StakePoolAccounts::new();
    stake_pool_accounts
        .initialize_stake_pool(&mut banks_client, &payer, &recent_blockhash)
        .await
        .unwrap();

    let validator_stake_account = ValidatorStakeAccount::new_with_target_authority(
        &stake_pool_accounts.deposit_authority,
        &stake_pool_accounts.stake_pool.pubkey(),
    );

    let user_stake_authority = Keypair::new();
    create_validator_stake_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &validator_stake_account.stake_pool,
        &validator_stake_account.stake_account,
        &validator_stake_account.vote.pubkey(),
        &user_stake_authority.pubkey(),
        &validator_stake_account.target_authority,
    )
    .await;

    let user = Keypair::new();
    // make stake account
    let user_stake = Keypair::new();
    let lockup = stake::Lockup::default();
    let authorized = stake::Authorized {
        staker: stake_pool_accounts.deposit_authority,
        withdrawer: stake_pool_accounts.deposit_authority,
    };
    create_independent_stake_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &user_stake,
        &authorized,
        &lockup,
    )
    .await;
    // make pool token account
    let user_pool_account = Keypair::new();
    create_token_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &user_pool_account,
        &stake_pool_accounts.pool_mint.pubkey(),
        &user.pubkey(),
    )
    .await
    .unwrap();

    let user_pool_account = user_pool_account.pubkey();
    let pool_tokens = get_token_balance(&mut banks_client, &user_pool_account).await;

    let tokens_to_burn = pool_tokens / 4;

    // Delegate tokens for burning
    delegate_tokens(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &user_pool_account,
        &user,
        &stake_pool_accounts.withdraw_authority,
        tokens_to_burn,
    )
    .await;

    // Create stake account to withdraw to
    let user_stake_recipient = Keypair::new();

    let new_authority = Pubkey::new_unique();

    let transaction_error = stake_pool_accounts
        .withdraw_stake(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &user_stake_recipient.pubkey(),
            &user_pool_account,
            &validator_stake_account.stake_account,
            &new_authority,
            tokens_to_burn,
        )
        .await
        .err()
        .unwrap();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(error_index),
        )) => {
            let program_error = error::StakePoolError::WrongStakeState as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!("Wrong error occurs while try to withdraw when stake acc not in stake state"),
    }
}

#[tokio::test]
async fn test_stake_pool_withdraw_from_unknown_validator() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let stake_pool_accounts = StakePoolAccounts::new();
    stake_pool_accounts
        .initialize_stake_pool(&mut banks_client, &payer, &recent_blockhash)
        .await
        .unwrap();

    let validator_stake_account = ValidatorStakeAccount::new_with_target_authority(
        &stake_pool_accounts.deposit_authority,
        &stake_pool_accounts.stake_pool.pubkey(),
    );
    validator_stake_account
        .create_and_delegate(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let user_stake = ValidatorStakeAccount::new_with_target_authority(
        &stake_pool_accounts.deposit_authority,
        &stake_pool_accounts.stake_pool.pubkey(),
    );
    user_stake
        .create_and_delegate(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let user_pool_account = Keypair::new();
    let user = Keypair::new();
    create_token_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &user_pool_account,
        &stake_pool_accounts.pool_mint.pubkey(),
        &user.pubkey(),
    )
    .await
    .unwrap();

    let user = Keypair::new();
    // make stake account
    let user_stake = Keypair::new();
    let lockup = stake::Lockup::default();
    let authorized = stake::Authorized {
        staker: stake_pool_accounts.deposit_authority,
        withdrawer: stake_pool_accounts.deposit_authority,
    };
    create_independent_stake_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &user_stake,
        &authorized,
        &lockup,
    )
    .await;
    // make pool token account
    let user_pool_account = Keypair::new();
    create_token_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &user_pool_account,
        &stake_pool_accounts.pool_mint.pubkey(),
        &user.pubkey(),
    )
    .await
    .unwrap();

    let user_pool_account = user_pool_account.pubkey();
    let pool_tokens = get_token_balance(&mut banks_client, &user_pool_account).await;

    let tokens_to_burn = pool_tokens / 4;

    // Delegate tokens for burning
    delegate_tokens(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &user_pool_account,
        &user,
        &stake_pool_accounts.withdraw_authority,
        tokens_to_burn,
    )
    .await;

    // Create stake account to withdraw to
    let user_stake_recipient = Keypair::new();

    let new_authority = Pubkey::new_unique();

    let transaction_error = stake_pool_accounts
        .withdraw_stake(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &user_stake_recipient.pubkey(),
            &user_pool_account,
            &validator_stake_account.stake_account,
            &new_authority,
            tokens_to_burn,
        )
        .await
        .err()
        .unwrap();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(error_index),
        )) => {
            let program_error = error::StakePoolError::ValidatorNotFound as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!("Wrong error occurs while try to do withdraw from unknown validator"),
    }
}

#[tokio::test]
async fn test_stake_pool_double_withdraw_to_the_same_account() {
    let (
        mut banks_client,
        payer,
        recent_blockhash,
        stake_pool_accounts,
        validator_stake_account,
        deposit_info,
        tokens_to_burn,
    ) = setup().await;

    // Create stake account to withdraw to
    let user_stake_recipient = Keypair::new();
    create_blank_stake_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &user_stake_recipient,
    )
    .await;

    let new_authority = Pubkey::new_unique();
    stake_pool_accounts
        .withdraw_stake(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &user_stake_recipient.pubkey(),
            &deposit_info.user_pool_account,
            &validator_stake_account.stake_account,
            &new_authority,
            tokens_to_burn,
        )
        .await
        .unwrap();

    let latest_blockhash = banks_client.get_recent_blockhash().await.unwrap();

    let transaction_error = stake_pool_accounts
        .withdraw_stake(
            &mut banks_client,
            &payer,
            &latest_blockhash,
            &user_stake_recipient.pubkey(),
            &deposit_info.user_pool_account,
            &validator_stake_account.stake_account,
            &new_authority,
            tokens_to_burn,
        )
        .await
        .err()
        .unwrap();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(_, error)) => {
            assert_eq!(error, InstructionError::InvalidAccountData);
        }
        _ => panic!("Wrong error occurs while try to do double withdraw"),
    }
}

#[tokio::test]
async fn test_stake_pool_withdraw_token_delegate_was_not_setup() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let stake_pool_accounts = StakePoolAccounts::new();
    stake_pool_accounts
        .initialize_stake_pool(&mut banks_client, &payer, &recent_blockhash)
        .await
        .unwrap();

    let validator_stake_account: ValidatorStakeAccount = simple_add_validator_stake_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &stake_pool_accounts,
    )
    .await;

    let deposit_info: DepositInfo = simple_deposit(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &stake_pool_accounts,
        &validator_stake_account,
    )
    .await;

    let tokens_to_burn = deposit_info.pool_tokens / 4;

    // Create stake account to withdraw to
    let user_stake_recipient = Keypair::new();
    create_blank_stake_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &user_stake_recipient,
    )
    .await;

    let new_authority = Pubkey::new_unique();
    let transaction_error = stake_pool_accounts
        .withdraw_stake(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &user_stake_recipient.pubkey(),
            &deposit_info.user_pool_account,
            &validator_stake_account.stake_account,
            &new_authority,
            tokens_to_burn,
        )
        .await
        .err()
        .unwrap();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(error_index),
        )) => {
            let program_error = TokenError::OwnerMismatch as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!(
            "Wrong error occurs while try to do withdraw without token delegation for burn before"
        ),
    }
}

#[tokio::test]
async fn test_stake_pool_withdraw_with_low_delegation() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let stake_pool_accounts = StakePoolAccounts::new();
    stake_pool_accounts
        .initialize_stake_pool(&mut banks_client, &payer, &recent_blockhash)
        .await
        .unwrap();

    let validator_stake_account: ValidatorStakeAccount = simple_add_validator_stake_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &stake_pool_accounts,
    )
    .await;

    let deposit_info: DepositInfo = simple_deposit(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &stake_pool_accounts,
        &validator_stake_account,
    )
    .await;

    let tokens_to_burn = deposit_info.pool_tokens / 4;

    // Delegate tokens for burning
    delegate_tokens(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &deposit_info.user_pool_account,
        &deposit_info.user,
        &stake_pool_accounts.withdraw_authority,
        1,
    )
    .await;

    // Create stake account to withdraw to
    let user_stake_recipient = Keypair::new();
    create_blank_stake_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &user_stake_recipient,
    )
    .await;

    let new_authority = Pubkey::new_unique();
    let transaction_error = stake_pool_accounts
        .withdraw_stake(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &user_stake_recipient.pubkey(),
            &deposit_info.user_pool_account,
            &validator_stake_account.stake_account,
            &new_authority,
            tokens_to_burn,
        )
        .await
        .err()
        .unwrap();

    match transaction_error {
        TransportError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(error_index),
        )) => {
            let program_error = TokenError::InsufficientFunds as u32;
            assert_eq!(error_index, program_error);
        }
        _ => panic!(
            "Wrong error occurs while try to do withdraw with not enough delegated tokens to burn"
        ),
    }
}
*/
