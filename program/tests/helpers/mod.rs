#![allow(dead_code)]

const META_LP_MINT_ACCOUNT:&str="4XMfT5tKJzXXzaw1u5VSUec5av6NBPAbiULnUpQ76YGX"; 
/// authority/owner for token accounts: LIQ_POOL_WSOL_ACCOUNT & LIQ_POOL_STSOL_ACCOUNT
const LIQ_POOL_ACCOUNT:&str="rxTBFFRfwcgx5YedbwLcKntCwMs9tJoQvzYmRnbpLKS"; 
/// wSOL token account 
const LIQ_POOL_WSOL_ACCOUNT:&str="7efxCKtTp5DDScftcEFSsxHPgqAk8WxuRAi786cg3qBZ"; //TODO
const LIQ_POOL_ST_SOL_ACCOUNT:&str="DWB8abtU8B2A3EQpY2dWxaHnPiVwXgHRQSaGBGAgdBj7"; //TODO

use solana_program::{hash::Hash, program_pack::Pack, pubkey::Pubkey, system_instruction};
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
    transport::TransportError,
};
use solana_vote_program::{self, vote_state::VoteState};
use spl_stake_pool::*;

pub const TEST_STAKE_AMOUNT: u64 = 100;

pub fn program_test() -> ProgramTest {
    ProgramTest::new(
        "spl_stake_pool",
        id(),
        processor!(processor::Processor::process),
    )
}

pub async fn get_account(banks_client: &mut BanksClient, pubkey: &Pubkey) -> Account {
    banks_client
        .get_account(*pubkey)
        .await
        .expect("account not found")
        .expect("account empty")
}

pub async fn create_mint(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    pool_mint: &Keypair,
    owner: &Pubkey,
) -> Result<(), TransportError> {
    let rent = banks_client.get_rent().await.unwrap();
    let mint_rent = rent.minimum_balance(spl_token::state::Mint::LEN);

    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &pool_mint.pubkey(),
                mint_rent,
                spl_token::state::Mint::LEN as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_mint(
                &spl_token::id(),
                &pool_mint.pubkey(),
                &owner,
                None,
                0,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, pool_mint], *recent_blockhash);
    banks_client.process_transaction(transaction).await?;
    Ok(())
}

pub async fn transfer(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    recipient: &Pubkey,
    amount: u64,
) {
    let mut transaction = Transaction::new_with_payer(
        &[system_instruction::transfer(
            &payer.pubkey(),
            recipient,
            amount,
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

pub async fn create_token_account(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    account: &Keypair,
    pool_mint: &Pubkey,
    owner: &Pubkey,

) -> Result<(), TransportError> {

    let rent = banks_client.get_rent().await.unwrap();
    let account_rent = rent.minimum_balance(spl_token::state::Account::LEN);

    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &account.pubkey(),
                100_000+account_rent,
                spl_token::state::Account::LEN as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_account(
                &spl_token::id(),
                &account.pubkey(),
                pool_mint,
                owner,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, account], *recent_blockhash);
    banks_client.process_transaction(transaction).await?;
    Ok(())
}

// CANT BE DONE HERE because mint AUTH is a PDA. Has to be done from the program
// pub async fn mint_token_to(
//     banks_client: &mut BanksClient,
//     payer: &Keypair,
//     recent_blockhash: &Hash,
//     account: &Keypair,
//     amount: u64,
//     pool_mint: &Pubkey,
//     owner: &Pubkey,

// ) -> Result<(), TransportError> {

//     println!("---about to mint_to {}",amount);
//     let mut transaction = Transaction::new_with_payer(
//         &[
//             spl_token::instruction::mint_to( //token_program_id: &Pubkey, mint_pubkey: &Pubkey, account_pubkey: &Pubkey, owner_pubkey: &Pubkey, signer_pubkeys: &[&Pubkey], amount: u64)(
//                 &spl_token::id(),
//                 pool_mint,
//                 &account.pubkey(),
//                 owner,
//                 &[],
//                 amount
//             )
//             .unwrap(),
//         ],
//         Some(&payer.pubkey()),
//     );
//     transaction.sign(&[payer, account], *recent_blockhash);
//     banks_client.process_transaction(transaction).await?;
//     println!("---after to mint_to {}",amount)
//     Ok(())
// }

pub async fn get_token_balance(banks_client: &mut BanksClient, token: &Pubkey) -> u64 {
    let token_account = banks_client
        .get_account(token.clone())
        .await
        .unwrap()
        .unwrap();
    let account_info: spl_token::state::Account =
        spl_token::state::Account::unpack_from_slice(token_account.data.as_slice()).unwrap();
    account_info.amount
}

pub async fn delegate_tokens(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    account: &Pubkey,
    owner: &Keypair,
    delegate: &Pubkey,
    amount: u64,
) {
    let mut transaction = Transaction::new_with_payer(
        &[spl_token::instruction::approve(
            &spl_token::id(),
            &account,
            &delegate,
            &owner.pubkey(),
            &[],
            amount,
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, owner], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

#[allow(clippy::too_many_arguments)]
pub async fn create_stake_pool(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    stake_pool: &Keypair,
    validator_stake_list: &Keypair,
    pool_mint: &Pubkey,
    pool_token_account: &Pubkey,
    owner: &Keypair,
    fee: &instruction::Fee,
) -> Result<(), TransportError> {
    let rent = banks_client.get_rent().await.unwrap();
    let rent_stake_pool = rent.minimum_balance(state::StakePool::LEN);
    let rent_validator_stake_list = rent.minimum_balance(state::ValidatorStakeList::LEN);
    let init_args = instruction::InitArgs { fee: *fee };

    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &stake_pool.pubkey(),
                rent_stake_pool,
                state::StakePool::LEN as u64,
                &id(),
            ),
            system_instruction::create_account(
                &payer.pubkey(),
                &validator_stake_list.pubkey(),
                rent_validator_stake_list,
                state::ValidatorStakeList::LEN as u64,
                &id(),
            ),
            instruction::initialize(
                &id(),
                &stake_pool.pubkey(),
                &owner.pubkey(),
                &validator_stake_list.pubkey(),
                pool_mint,
                pool_token_account,
                &spl_token::id(),
                init_args,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(
        &[payer, stake_pool, validator_stake_list, owner],
        *recent_blockhash,
    );
    banks_client.process_transaction(transaction).await?;
    Ok(())
}

pub async fn create_vote(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    vote: &Keypair,
) {
    let rent = banks_client.get_rent().await.unwrap();
    let rent_voter = rent.minimum_balance(VoteState::size_of());

    let mut transaction = Transaction::new_with_payer(
        &[system_instruction::create_account(
            &payer.pubkey(),
            &vote.pubkey(),
            rent_voter,
            VoteState::size_of() as u64,
            &solana_vote_program::id(),
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&vote, payer], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

pub async fn create_independent_stake_account(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    stake: &Keypair,
    authorized: &stake::Authorized,
    lockup: &stake::Lockup,
) -> u64 {
    let rent = banks_client.get_rent().await.unwrap();
    let lamports =
        rent.minimum_balance(std::mem::size_of::<stake::StakeState>()) + TEST_STAKE_AMOUNT;

    let mut transaction = Transaction::new_with_payer(
        &stake::create_account(
            &payer.pubkey(),
            &stake.pubkey(),
            authorized,
            lockup,
            lamports,
        ),
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, stake], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    lamports
}

pub async fn create_account(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    account: &Keypair,
) -> Result<(), TransportError> { 
    
    let rent = banks_client.get_rent().await.unwrap();
    let lamports = rent.minimum_balance(state::StakePool::LEN);

    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &account.pubkey(),
                lamports,
                spl_token::state::Account::LEN as u64,
                &id(),
            ),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, account], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    Ok(())
}

pub async fn create_blank_stake_account(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    stake: &Keypair,
) -> u64 {
    let rent = banks_client.get_rent().await.unwrap();
    let lamports = rent.minimum_balance(std::mem::size_of::<stake::StakeState>()) + 1;

    let mut transaction = Transaction::new_with_payer(
        &[system_instruction::create_account(
            &payer.pubkey(),
            &stake.pubkey(),
            lamports,
            std::mem::size_of::<stake::StakeState>() as u64,
            &stake::id(),
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, stake], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    lamports
}

pub async fn create_validator_stake_account(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    stake_pool: &Pubkey,
    stake_account: &Pubkey,
    validator: &Pubkey,
    stake_authority: &Pubkey,
    withdraw_authority: &Pubkey,
) {
    let mut transaction = Transaction::new_with_payer(
        &[
            instruction::create_validator_stake_account(
                &id(),
                &stake_pool,
                &payer.pubkey(),
                &stake_account,
                &validator,
                &stake_authority,
                &withdraw_authority,
                &solana_program::system_program::id(),
                &stake::id(),
            )
            .unwrap(),
            system_instruction::transfer(&payer.pubkey(), &stake_account, TEST_STAKE_AMOUNT),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

pub async fn delegate_stake_account(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    stake: &Pubkey,
    authorized: &Keypair,
    vote: &Pubkey,
) {
    let mut transaction = Transaction::new_with_payer(
        &[stake::delegate_stake(&stake, &authorized.pubkey(), &vote)],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, authorized], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

pub async fn authorize_stake_account(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    stake: &Pubkey,
    authorized: &Keypair,
    new_authorized: &Pubkey,
    stake_authorize: stake::StakeAuthorize,
) {
    let mut transaction = Transaction::new_with_payer(
        &[stake::authorize(
            &stake,
            &authorized.pubkey(),
            &new_authorized,
            stake_authorize,
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, authorized], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

pub struct ValidatorStakeAccount {
    pub stake_account: Pubkey,
    pub target_authority: Pubkey,
    pub vote: Keypair,
    pub stake_pool: Pubkey,
}

impl ValidatorStakeAccount {
    pub fn new_with_target_authority(authority: &Pubkey, stake_pool: &Pubkey) -> Self {
        let validator = Keypair::new();
        let (stake_account, _) = processor::Processor::find_stake_address_for_validator(
            &id(),
            &validator.pubkey(),
            stake_pool,
        );
        ValidatorStakeAccount {
            stake_account,
            target_authority: *authority,
            vote: validator,
            stake_pool: *stake_pool,
        }
    }

    pub async fn create_and_delegate(
        &self,
        mut banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,
    ) {
        // make stake account
        let user_stake_authority = Keypair::new();
        create_validator_stake_account(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.stake_pool,
            &self.stake_account,
            &self.vote.pubkey(),
            &user_stake_authority.pubkey(),
            &self.target_authority,
        )
        .await;

        create_vote(&mut banks_client, &payer, &recent_blockhash, &self.vote).await;
        delegate_stake_account(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.stake_account,
            &user_stake_authority,
            &self.vote.pubkey(),
        )
        .await;

        authorize_stake_account(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.stake_account,
            &user_stake_authority,
            &self.target_authority,
            stake::StakeAuthorize::Staker,
        )
        .await;
    }
}

pub struct StakePoolAccounts {
    pub stake_pool: Keypair,
    pub validator_stake_list: Keypair,
    pub pool_mint: Keypair,
    pub pool_fee_account: Keypair,
    pub owner: Keypair,
    pub withdraw_authority: Pubkey,
    pub deposit_authority: Pubkey,
    pub fee: instruction::Fee,

    pub liq_pool_state_acc: Keypair,
    pub liq_pool_authority: Pubkey,
    pub meta_lp_mint_acc: Keypair,
    pub liq_pool_wsol_acc: Keypair,
    pub liq_pool_st_sol_acc: Keypair,
}

impl StakePoolAccounts {

    pub fn new() -> Self {

        let stake_pool = Keypair::new();
        let validator_stake_list = Keypair::new();
        let stake_pool_address = &stake_pool.pubkey();

        let (withdraw_authority, _) = Pubkey::find_program_address(
            &[&stake_pool_address.to_bytes()[..32], b"withdraw"],
            &id(),
        );
        let (deposit_authority, _) = Pubkey::find_program_address(
            &[&stake_pool_address.to_bytes()[..32], b"deposit"],
            &id(),
        );

        let pool_mint = Keypair::new();
        let pool_fee_account = Keypair::new();
        let owner = Keypair::new();

        let liq_pool_state_acc = Keypair::new();
        let meta_lp_mint_acc = Keypair::new();
        let liq_pool_wsol_acc = Keypair::new();
        let liq_pool_st_sol_acc = Keypair::new();

        // let (liq_pool_authority, _) = Pubkey::find_program_address(
        //     &[&liq_pool_state_acc.to_bytes()[..32], b"withdraw"],
        //     &id(),
        // );
        // let (liq_pool_authority, bump) = Pubkey::find_program_address(
        //     &[b"escrow"],
        //     &id(),
        // );
        println!("id {}",&id());
        println!("&liq_pool_state_acc.pubkey().to_bytes()[..32] {:?}",&liq_pool_state_acc.pubkey().to_bytes()[..32]);
        let (liq_pool_authority,_) = Pubkey::find_program_address(&[&liq_pool_state_acc.pubkey().to_bytes()[..32] ,b"authority"], &id());
        //println!("liq_pool_authority {} bump {}",liq_pool_authority,bump);
        println!("liq_pool_authority {}",liq_pool_authority);

        Self {
            stake_pool,
            validator_stake_list,
            pool_mint,
            pool_fee_account,
            owner,
            withdraw_authority,
            deposit_authority,
            fee: instruction::Fee {
                numerator: 1,
                denominator: 100,
            },
            liq_pool_state_acc,
            liq_pool_authority,
            meta_lp_mint_acc,
            liq_pool_wsol_acc,
            liq_pool_st_sol_acc,
            }
    }

    pub fn calculate_fee(&self, amount: u64) -> u64 {
        amount * self.fee.numerator / self.fee.denominator
    }

    pub async fn initialize_stake_pool(
        &self,
        mut banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,

    ) -> Result<(), TransportError> {

        create_mint(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.pool_mint,
            &self.withdraw_authority,
        )
        .await?;
        create_token_account(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.pool_fee_account,
            &self.pool_mint.pubkey(),
            &self.owner.pubkey(),
        )
        .await?;
        create_stake_pool(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.stake_pool,
            &self.validator_stake_list,
            &self.pool_mint.pubkey(),
            &self.pool_fee_account.pubkey(),
            &self.owner,
            &self.fee,
        )
        .await?;

        //liq pool state account (not used yet)
        create_account(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.liq_pool_state_acc,
        )
        .await?;

        //--LIQ POOL

        // lp wsol acc (1st side of the liq-pool)
        create_token_account(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.liq_pool_wsol_acc,
            &String::from(W_SOL_1111111_MINT_ACCOUNT).parse().unwrap(),
            &self.liq_pool_authority,
        )
        .await?;

        // lp st_sol acc (2nd side of the liq-pool)
        create_token_account(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.liq_pool_st_sol_acc,
            &self.pool_mint.pubkey(),
            &self.liq_pool_authority,
        )
        .await?;

        // meta_lp mint
        create_mint(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.meta_lp_mint_acc,
            &self.liq_pool_authority,
        )
        .await?;

        Ok(())
    }


    pub async fn deposit_stake(
        &self,
        banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,
        stake: &Pubkey,
        pool_account: &Pubkey,
        validator_stake_account: &Pubkey,
    ) -> Result<(), TransportError> {
        let mut transaction = Transaction::new_with_payer(
            &[instruction::deposit(
                &id(),
                &self.stake_pool.pubkey(),
                &self.validator_stake_list.pubkey(),
                &self.deposit_authority,
                &self.withdraw_authority,
                stake,
                validator_stake_account,
                pool_account,
                &self.pool_fee_account.pubkey(),
                &self.pool_mint.pubkey(),
                &spl_token::id(),
                &stake::id(),
            )
            .unwrap()],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[payer], *recent_blockhash);
        banks_client.process_transaction(transaction).await?;
        Ok(())
    }

    pub async fn withdraw_stake(
        &self,
        banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,
        stake_recipient: &Pubkey,
        pool_account: &Pubkey,
        validator_stake_account: &Pubkey,
        recipient_new_authority: &Pubkey,
        amount: u64,
    ) -> Result<(), TransportError> {
        let mut transaction = Transaction::new_with_payer(
            &[instruction::withdraw(
                &id(),
                &self.stake_pool.pubkey(),
                &self.validator_stake_list.pubkey(),
                &self.withdraw_authority,
                validator_stake_account,
                stake_recipient,
                recipient_new_authority,
                pool_account,
                &self.pool_mint.pubkey(),
                &spl_token::id(),
                &stake::id(),
                amount,
            )
            .unwrap()],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[payer], *recent_blockhash);
        banks_client.process_transaction(transaction).await?;
        Ok(())
    }

    pub async fn add_validator_stake_account(
        &self,
        banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,
        stake: &Pubkey,
        pool_account: &Pubkey,
    ) -> Option<TransportError> {
        let mut transaction = Transaction::new_with_payer(
            &[instruction::add_validator_stake_account(
                &id(),
                &self.stake_pool.pubkey(),
                &self.owner.pubkey(),
                &self.deposit_authority,
                &self.withdraw_authority,
                &self.validator_stake_list.pubkey(),
                stake,
                pool_account,
                &self.pool_mint.pubkey(),
                &spl_token::id(),
                &stake::id(),
            )
            .unwrap()],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[payer, &self.owner], *recent_blockhash);
        banks_client.process_transaction(transaction).await.err()
    }

    pub async fn remove_validator_stake_account(
        &self,
        banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,
        stake: &Pubkey,
        pool_account: &Pubkey,
        new_authority: &Pubkey,
    ) -> Option<TransportError> {
        let mut transaction = Transaction::new_with_payer(
            &[instruction::remove_validator_stake_account(
                &id(),
                &self.stake_pool.pubkey(),
                &self.owner.pubkey(),
                &self.withdraw_authority,
                &new_authority,
                &self.validator_stake_list.pubkey(),
                stake,
                pool_account,
                &self.pool_mint.pubkey(),
                &spl_token::id(),
                &stake::id(),
            )
            .unwrap()],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[payer, &self.owner], *recent_blockhash);
        banks_client.process_transaction(transaction).await.err()
    }
}

pub async fn simple_add_validator_stake_account(

    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    stake_pool_accounts: &StakePoolAccounts,

) -> ValidatorStakeAccount {

    let user_stake = ValidatorStakeAccount::new_with_target_authority(
        &stake_pool_accounts.deposit_authority,
        &stake_pool_accounts.stake_pool.pubkey(),
    );
    user_stake
        .create_and_delegate(banks_client, &payer, &recent_blockhash)
        .await;

    let user_pool_account = Keypair::new();
    let user = Keypair::new();
    create_token_account(
        banks_client,
        &payer,
        &recent_blockhash,
        &user_pool_account,
        &stake_pool_accounts.pool_mint.pubkey(),
        &user.pubkey(),
    )
    .await
    .unwrap();

    let error = stake_pool_accounts
        .add_validator_stake_account(
            banks_client,
            &payer,
            &recent_blockhash,
            &user_stake.stake_account,
            &user_pool_account.pubkey(),
        )
        .await;
    assert!(error.is_none());

    println!("-- end simple_add_validator_stake_account");

    user_stake
}

pub struct DepositInfo {
    pub user: Keypair,
    pub user_pool_account: Pubkey,
    pub stake_lamports: u64,
    pub pool_tokens: u64,
}

pub async fn simple_deposit(
    banks_client: &mut BanksClient,
    payer: &Keypair, payer_is_user: bool,
    recent_blockhash: &Hash,
    stake_pool_accounts: &StakePoolAccounts,
    validator_stake_account: &ValidatorStakeAccount,
) -> DepositInfo {
    let user = Keypair::new();
    // make stake account
    let user_stake = Keypair::new();
    let lockup = stake::Lockup::default();
    let authorized = stake::Authorized {
        staker: stake_pool_accounts.deposit_authority,
        withdrawer: stake_pool_accounts.deposit_authority,
    };
    let stake_lamports = create_independent_stake_account(
        banks_client,
        payer,
        recent_blockhash,
        &user_stake,
        &authorized,
        &lockup,
    )
    .await;
    // make pool token account
    let user_pool_account = Keypair::new();
    let owner = &if payer_is_user {payer.pubkey()} else {user.pubkey()};
    create_token_account(
        banks_client,
        payer,
        recent_blockhash,
        &user_pool_account,
        &stake_pool_accounts.pool_mint.pubkey(),
        owner,
    )
    .await
    .unwrap();

    stake_pool_accounts
        .deposit_stake(
            banks_client,
            payer,
            recent_blockhash,
            &user_stake.pubkey(),
            &user_pool_account.pubkey(),
            &validator_stake_account.stake_account,
        )
        .await
        .unwrap();

    let user_pool_account = user_pool_account.pubkey();
    let pool_tokens = get_token_balance(banks_client, &user_pool_account).await;

    return DepositInfo {
        user,
        user_pool_account,
        stake_lamports,
        pool_tokens,
    }
}

pub async fn prepare_wsol_deposit(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,

) -> DepositInfo {

    println!("-- enter prepare_wsol_deposit");
    let user = Keypair::new();
    // make wsol token account
    let user_wsol_account = Keypair::new();
    create_token_account(
        banks_client,
        payer,
        recent_blockhash,
        &user_wsol_account,
        &String::from(W_SOL_1111111_MINT_ACCOUNT).parse().unwrap(),
        &payer.pubkey(),
    )
    .await
    .unwrap();

    let user_wsol_account = user_wsol_account.pubkey();
    let wsol_tokens = get_token_balance(banks_client, &user_wsol_account).await;
    
    println!("-- exit prepare_wsol_deposit");

    return DepositInfo {
        user,
        user_pool_account:user_wsol_account,
        stake_lamports:0,
        pool_tokens: wsol_tokens,
    }
}


pub async fn prepare_st_sol_deposit(
    banks_client: &mut BanksClient,
    st_sol_token_mint: &Pubkey,
    payer: &Keypair,
    recent_blockhash: &Hash,

) -> DepositInfo {

    println!("-- enter prepare_st_sol_deposit");
    let user = Keypair::new();
    // make wsol token account
    let user_st_sol_account = Keypair::new();
    create_token_account(
        banks_client,
        payer,
        recent_blockhash,
        &user_st_sol_account,
        &st_sol_token_mint,
        &payer.pubkey(),
    )
    .await
    .unwrap();

    let st_sol_tokens = get_token_balance(banks_client, &user_st_sol_account.pubkey()).await;
    println!("-- exit prepare_st_sol_deposit {}",st_sol_tokens);

    return DepositInfo {
        user,
        user_pool_account:user_st_sol_account.pubkey(),
        stake_lamports:0,
        pool_tokens: st_sol_tokens,
    }
}
