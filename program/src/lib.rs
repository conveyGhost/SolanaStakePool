#![deny(missing_docs)]

//! A program for managing a meta-staking-pool with a liquidity-pool to allow immediate unstaking

/// wSOL mint account
pub const W_SOL_1111111_MINT_ACCOUNT:&str="So11111111111111111111111111111111111111112";


//---------------------
//Values for TESTNET
//---------------------
/// testnet account code deployment
pub const META_POOL_PROGRAM_ACCOUNT_ID:&str="E2wGYXEPw46FdJWL1MfRoN3JhQY4w6Dmaz9s4ehV2483";
/*
Creating mint 21ofzqmgounc8bX4CK6j3Ff4zjvX6GmRykUnJAU96zKz
Creating pool fee collection account EG42YnCDMx1ham3NVryGM71yiCo3zNSyJ1ktPvguFtqE
Creating stake pool C3WQybyZc45bhRP4PJnM7JhKQFXmqQR5eWr8n8Lxjgex
Signature: 5AdVComuwVXbcyD2MmXNMgAKGx7aE1o8eXXzCgvmxjdgoPmr7Hd5xL28NARTH88PGbGb3ZzxbkjYKcpgic5fUDTY
*/
/// stSOL mint
pub const ST_SOL_MINT_ACCOUNT:&str="21ofzqmgounc8bX4CK6j3Ff4zjvX6GmRykUnJAU96zKz";
/// sell stSOL fee
pub const POOL_FEE_COLLECTION_ACCOUNT:&str="EG42YnCDMx1ham3NVryGM71yiCo3zNSyJ1ktPvguFtqE";
/// stake-pool
pub const POOL_ACCOUNT_ID:&str="C3WQybyZc45bhRP4PJnM7JhKQFXmqQR5eWr8n8Lxjgex";
//Creating mint HVrdtDVPsWHDec5YZQoQ4Zf9Ew4EeTcxLWoCaXAU4Bib
//Creating liquidity pool FX1c3XJwtjvGQu9ZyGGBQn78EjarNfPcSyyKvCapq6iH
/// $METALP mint
pub const META_LP_MINT_ACCOUNT:&str="HVrdtDVPsWHDec5YZQoQ4Zf9Ew4EeTcxLWoCaXAU4Bib"; 
/// LIQUIDITY POOL containing wSOL/stSOL
pub const LIQ_POOL_ACCOUNT:&str="FX1c3XJwtjvGQu9ZyGGBQn78EjarNfPcSyyKvCapq6iH"; 
//---------------------


pub mod error;
pub mod instruction;
pub mod processor;
pub mod stake;
pub mod state;

/// Current program version
pub const PROGRAM_VERSION: u8 = 1;

#[cfg(not(feature = "no-entrypoint"))]
pub mod entrypoint;

// Export current sdk types for downstream users building with a different sdk version
pub use solana_program;

solana_program::declare_id!("E2wGYXEPw46FdJWL1MfRoN3JhQY4w6Dmaz9s4ehV2483");
