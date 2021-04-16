# Meta CLI

### Meta Pool program command-line utility

A basic command-line for creating and using the MetaPool. The meta pool is an extension of the SPL-Stake-Pool.

This is the CLI utility. The main doc is here: https://github.com/SolAutoStake/StakePool

This rust-coded CLI expands on the SPL-Stake-Pool CLI. It adds the following commands:

### create-liq-pool

Status: Working:

This command must be *run once*-. It creates the liquidity pool.
The accounts created by the run must be included into the CLI and the CLI recompiled so the rest of the commands act on the created accounts.
For the testnet demo, it has been run already and the accounts are defined as constants at https://github.com/SolAutoStake/StakePool/blob/main/metacli/src/main.rs

### add-liquidity

Status: Working:

This command can be used by *advanced users* to add liquidity to the Liquidity pool. You specify where to take wSOL from. The cli will create a new token account for the signer cotaining $META-LP: the token presenting your share of the liquidity pool.

```
$ ./meta add-liquidity --help
meta-add-liquidity 
Add wSOL amount to wSOL/stSOL Liquidity pool

USAGE:
    meta add-liquidity [FLAGS] [OPTIONS] <AMOUNT> --source <ADDRESS>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Show additional information

OPTIONS:
    -C, --config <PATH>       Configuration file to use [default: /home/lucio/.config/solana/cli/config.yml]
    -s, --source <ADDRESS>    wSOL token account to take wSOL from. Must be owned by the signer.

ARGS:
    <AMOUNT>    Amount of wSOL to add.
```
  
### sell

This command can be used by *advanced users* to sell st-SOL from the command line. Other users are encouraged to use the Web App UI to sell stSOL. You specify where to take stSOL from. The cli will create a new token account for the signer cotaining wSOL according to value of the stSOL sold minus a fee (3% by default)

```
$ ./meta sell --help
meta-sell 
Sell stSOL for wSOL using the liquidity pool

USAGE:
    meta sell [FLAGS] [OPTIONS] <AMOUNT> --source <ADDRESS>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Show additional information

OPTIONS:
    -C, --config <PATH>       Configuration file to use [default: /home/lucio/.config/solana/cli/config.yml]
    -s, --source <ADDRESS>    stSOL token account to take stSOL from. Must be owned by the signer.

ARGS:
    <AMOUNT>    Amount of stSOL to sell.
```

### remove-liquidity

Status: WIP

This command can be used by *advanced users* to remove liquidity from the Liquidity pool. You specify where to take $META-LP from. The cli will burn $META-LP and transfer you the corresponding wSOL & stSOL fromthe pool. The value of what you remove is always greater to the value you added originally. The added value comes from sell fees (3%) and rewards on stSOL on the pool.
