# MetaPool

## Overview

This a MVP for the Solana Feb-2021 Hackathon

For this project we extended the SPL-stake-pool into our own "Meta-Pool"

This project includes:

* A [Web App UI for the Meta-Pool](https://github.com/SolAutoStake/sol-stakepool-interface) including:
  * Stake accounts management for the SPL-stake-pool
  * Sell stSOL functionality for immediate unstake

* a [Backend Solana Program, the "Meta-Pool"](https://github.com/SolAutoStake/StakePool/tree/main/program) extending the SPL-stake-pool by adding a *Liquidity Pool* and with that the possibility of "selling" your staking pool tokens (stSOL) and receive wSOL immediately. By using this functionality, users can skip the cool-down period and get their wSOL out of the staking-pool in a single step.

* a new [CLI tool, called *meta*](https://github.com/SolAutoStake/StakePool/tree/main/metacli) allowing the same functionality of the SPL-stake-pool CLI, plus:
  * A command to create the liquidity pool: `create-liq pool`
  * A command to `add-liquidity`
  * A command to `sell-stsol` with the command line
  * A command to `remove-liquidity` (WIP)

### Value Added

#### Helps users distribute stake between validators
* The web app UI will use stats and validator metrics to choose different validators for the users to stake. The user receives stake-pool tokens (stSOL) so they can trade the token and they don't need to track dozens of stake-accounts. 

#### Provides immediate unstake 
* The backend solana program adds a liquidity pool holding wSOL and stSOL (stake-pool tokens). The users can sell their stSOL for immediate unstaking, receiving wSOL. This also creates the opportunity for advanced users to become liquidity providers, providing the immediate unstaking service and earning fees on each sell. 

#### Contributes to decentralization 
* This program helps decentralization by distributing users' staking-accounts to several validators.

#### Creates a new Liquidity Pool
* This program includes a liquidity pool and the opportunity for liquidity providers to earn fees. The liquidity-pool is the wSOL/stSOL pool that provides immediate unstake (sell stSOL) for a fee.

## stSOL Tokens

This program extends the SPL-stake-pool program. We named our stake-pool tokens: **stSOL**, for *staked SOL*

stSOLs represent staked SOL, and can be sold for wSOL in the wSOL/stSOL Liquidity Pool (paying a fee to skip the unstaking cool-down period). The value of your stSOL holdings is automatically incremented each epoch when staking rewards are paid. 

## Immediate Unstake

Users wanting to unstake skipping the 2 day cool-down period can do so in the *wSOL/stSOL Liquidity Pool*.

In the Liquidity Pool:
 * Users providing liquidity can earn fees on each sell
 * Users wanting to unstake without the cool-down period can do so for a fee.

The *wSOL/stSOL Liquidity Pool* is a one-sided Liquidity pool. Liquidity providers add only wSOL to the Liq-Pool. The Liq-Pool allows other users to SELL stSOL for wSOL at a discounted price. The discount represents how much users value skipping the 2 day cool-down period to receive their funds. In future enhancements we plan to compute fee % based on a curve to incentivize liquidity providers when the wSOL side of the pool is low.

## Standard stake-pool

This contract extends the SPL-stake-pool, so users can utlize it as they do the standard SPL-stake-pool.

## User stories:
### Alice 

Alice wants to stake her SOL with low risk, and also help the community by promoting validator diversification. 
Alice uses the meta-pool to stake.

Alice makes several stake-deposits. Each time the web-app UI redirects the stake to a different validator. Alice receives stSOL tokens, all in a single account, representing her share of the stake-pool.

She starts earning staking rewards on her stSOL. By holding stSOL she also has the possibility to sell some of her stSOL skipping the waiting period if the need arises.

### Bob 

Bob already has deposited stake and now holds 10,000 stSOL earning rewards. 

Bob needs to unstake 1,000 SOL to use in an emergency. He can’t wait 2 days to get his SOL. 

Bob sells 1,030 stSOL for 1,000 wSOL. He sells at a 3% discounted price to get the SOL immediately.
Bob gets wSOL in his account. Bob can use his SOL immediately.

### Carol 

Carol is an investor. She wants to provide liquidity for the wSOL/stSOL pool, earning operation fees.
Carol uses the meta-cli tool, to deposit 7,000 wSOL in the liquidity pool.
She is the first in the pool, so she gets 7,000 $METALP tokens.

Bob swaps 1,030 stSOL for 1,000 wSOL. He sells at a 3% discounted price to get the SOL immediately. The Liq-Pool delivers 1,000 wSOL to Bob and acquires 1,030 stSOL from Bob. The new value of the $META-LP is now 7,030 SOL (6,000 wSOL + 1,030 stSOL), 

Carol $META-LP holding value have increased, and now she owns some stSOL via the Liq-Pool. Carol can eventually withdraw all her liquidity retrieving 6,000 wSOL and 1,030 stSOL

