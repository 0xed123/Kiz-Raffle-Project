use anchor_lang::prelude::*;
use anchor_spl::{
    token::{self, Token, TokenAccount, Transfer},
};
use solana_program::pubkey::Pubkey;

pub mod account;
pub mod constants;
pub mod error;
pub mod utils;

use account::*;
use constants::*;
use error::*;
use utils::*;

declare_id!("HYpCYwetRSXdjVuvbZi9g4mTz2Ng2XSrdcPx1fpwHnto");

#[program]
pub mod raffle {
    use super::*;
    /**
     * @dev Initialize the project
     */
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let global_authority = &mut ctx.accounts.global_authority;
        global_authority.super_admin = ctx.accounts.admin.key();
        Ok(())
    }

    /**
     * @dev Create new raffle with new arguements
     * @Context has admin, global_authority accounts.
     * and zero-account Raffle, owner's nft ATA and global_authority's nft ATA
     * and nft mint address
     * @param global_bump: global authority's bump
     * @param ticket_price_ape: ticket price by ape
     * @param ticket_price_sol: ticket price by sol
     * @param end_timestamp: the end time of raffle
     * @param whitelisted: if 1: winner will get the nft, if 0: winners get whitelist spot
     * @param max_entrants: entrants amount to take part in this raffle
     */
    pub fn create_raffle(
        ctx: Context<CreateRaffle>,
        ticket_price_ape: u64,
        ticket_price_sol: u64,
        end_timestamp: i64,
        whitelisted: u64,
        max_entrants: u64,
    ) -> Result<()> {
        let mut raffle = ctx.accounts.raffle.load_init()?;
        let timestamp = Clock::get()?.unix_timestamp;
        msg!("Timestamp: {}", timestamp);

        if max_entrants > 5000 {
            return Err(RaffleError::MaxEntrantsTooLarge.into());
        }
        if timestamp > end_timestamp {
            return Err(RaffleError::EndTimeError.into());
        }

        // Transfer NFT to the PDA
        let src_token_account_info = &mut &ctx.accounts.owner_temp_nft_account;
        let dest_token_account_info = &mut &ctx.accounts.dest_nft_token_account;
        let token_program = &mut &ctx.accounts.token_program;

        let cpi_accounts = Transfer {
            from: src_token_account_info.to_account_info().clone(),
            to: dest_token_account_info.to_account_info().clone(),
            authority: ctx.accounts.admin.to_account_info().clone(),
        };
        token::transfer(
            CpiContext::new(token_program.clone().to_account_info(), cpi_accounts),
            1,
        )?;

        raffle.creator = ctx.accounts.admin.key();
        raffle.nft_mint = ctx.accounts.nft_mint_address.key();
        raffle.ticket_price_ape = ticket_price_ape;
        raffle.ticket_price_sol = ticket_price_sol;
        raffle.start_timestamp = timestamp;
        raffle.end_timestamp = end_timestamp;
        raffle.max_entrants = max_entrants;
        raffle.whitelisted = whitelisted;

        Ok(())
    }

    /**
     * @dev Update Raffle Period
     * @Context has admin and raffle accounts
     * @param end_timestamp: new end_timestamp
     */
    pub fn update_raffle_period(ctx: Context<UpdateRaffle>, end_timestamp: i64) -> Result<()> {
        let timestamp = Clock::get()?.unix_timestamp;
        let mut raffle = ctx.accounts.raffle.load_mut()?;
        if timestamp > end_timestamp {
            return Err(RaffleError::EndTimeError.into());
        }

        if timestamp > raffle.end_timestamp {
            return Err(RaffleError::RaffleEnded.into());
        }
        // if raffle.creator != ctx.accounts.admin.key() {
        //     return Err(RaffleError::NotCreator.into());
        // }

        raffle.end_timestamp = end_timestamp;

        Ok(())
    }

    /**
     * @dev Buy tickets functions
     * @Context has buyer and raffle's account.
     * global_authority and creator address and their reap token ATAs
     * @param global_bump: global_authority's bump
     * @param amount: the amount of the tickets
     * @param is_whitelisted: the whitelisted wallet: 0  101% whitelisted wallet: 1, no whitelisted wallet: 2
     */
    pub fn buy_tickets(
        ctx: Context<BuyTickets>, 
        amount: u64,
        is_whitelisted: u64
    ) -> Result<()> {
        let timestamp = Clock::get()?.unix_timestamp;
        let mut raffle = ctx.accounts.raffle.load_mut()?;

        if timestamp > raffle.end_timestamp {
            return Err(RaffleError::RaffleEnded.into());
        }
        if raffle.count + amount > raffle.max_entrants {
            return Err(RaffleError::NotEnoughTicketsLeft.into());
        }

        let total_amount_ape = amount * raffle.ticket_price_ape;
        let total_amount_sol = amount * raffle.ticket_price_sol;

        if ctx.accounts.buyer.to_account_info().lamports() < total_amount_sol {
            return Err(RaffleError::NotEnoughSOL.into());
        }
        if raffle.count == 0 {
            raffle.no_repeat = 1;
        } else {
            let mut index: u64 = 0;
            for i in 0..raffle.count {
                if raffle.entrants[i as usize] == ctx.accounts.buyer.key() {
                    index = i + 1 as u64;
                }
            }
            if index != 0 {
                raffle.no_repeat += 1;
            }
        }

        for _ in 0..amount {
            raffle.append(ctx.accounts.buyer.key());
        }

        let src_account_info = &mut &ctx.accounts.user_token_account;
        let dest_account_info = &mut &ctx.accounts.creator_token_account;
        let admin_dest_account_info = &mut &ctx.accounts.admin_token_account;

        let token_program = &mut &ctx.accounts.token_program;
       
        if total_amount_ape > 0 {

            let cpi_accounts = Transfer {
                from: src_account_info.to_account_info().clone(),
                to: dest_account_info.to_account_info().clone(),
                authority: ctx.accounts.buyer.to_account_info().clone(),
            };
            token::transfer(
                CpiContext::new(token_program.clone().to_account_info(), cpi_accounts),
                total_amount_ape,
            )?;
            
            let cpi_accounts = Transfer {
                from: src_account_info.to_account_info().clone(),
                to: admin_dest_account_info.to_account_info().clone(),
                authority: ctx.accounts.buyer.to_account_info().clone(),
            };
            token::transfer(
                CpiContext::new(token_program.clone().to_account_info(), cpi_accounts),
                total_amount_ape * is_whitelisted / 100,
            )?;
        }

        if total_amount_sol > 0 {
            sol_transfer_user(
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.creator.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                total_amount_sol,
            )?;
                
            sol_transfer_user(
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.admin.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                total_amount_sol * is_whitelisted / 100,
            )?;
        }

        Ok(())
    }

    /**
     * @dev Reaveal winner function
     * @Context has buyer and raffle account address
     */
    pub fn reveal_winner(ctx: Context<RevealWinner>) -> Result<()> {
        let timestamp = Clock::get()?.unix_timestamp;
        let mut raffle = ctx.accounts.raffle.load_mut()?;

        if timestamp < raffle.end_timestamp {
            return Err(RaffleError::RaffleNotEnded.into());
        }

        if raffle.whitelisted == 2 {
            return Err(RaffleError::InvalidToReveal.into());
        }

        let (player_address, _bump) = Pubkey::find_program_address(
            &[
                RANDOM_SEED.as_bytes(),
                timestamp.to_string().as_bytes(),
            ],
            &raffle::ID,
        );

        let char_vec: Vec<char> = player_address.to_string().chars().collect();
        let mut mul = 1;
        for i in 0..7 {
            mul *= u64::from(char_vec[i as usize]);
        }
        mul += u64::from(char_vec[7]);

        let winner_index = mul % raffle.count;

        raffle.whitelisted = 2;
        raffle.winner = raffle.entrants[winner_index as usize];

        Ok(())
    }

    /**
     * @dev Claim reward function
     * @Context has claimer and global_authority account
     * raffle account and the nft ATA of claimer and global_authority.
     * @param global_bump: the global_authority's bump
     */
    pub fn claim_reward(ctx: Context<ClaimReward>, global_bump: u8) -> Result<()> {
        let timestamp = Clock::get()?.unix_timestamp;
        let mut raffle = ctx.accounts.raffle.load_mut()?;

        if timestamp < raffle.end_timestamp {
            return Err(RaffleError::RaffleNotEnded.into());
        }

        if raffle.winner != ctx.accounts.claimer.key() {
            return Err(RaffleError::NotWinner.into());
        }

        // Transfer NFT to the winner's wallet
        let src_token_account = &mut &ctx.accounts.src_nft_token_account;
        let dest_token_account = &mut &ctx.accounts.claimer_nft_token_account;
        let token_program = &mut &ctx.accounts.token_program;
        let seeds = &[GLOBAL_AUTHORITY_SEED.as_bytes(), &[global_bump]];
        let signer = &[&seeds[..]];
        let cpi_accounts = Transfer {
            from: src_token_account.to_account_info().clone(),
            to: dest_token_account.to_account_info().clone(),
            authority: ctx.accounts.global_authority.to_account_info(),
        };
        token::transfer(
            CpiContext::new_with_signer(
                token_program.clone().to_account_info(),
                cpi_accounts,
                signer,
            ),
            1,
        )?;
        raffle.whitelisted = 4;
    
        Ok(())
    }

    /**
     * @dev Withdraw NFT function
     * @Context has claimer and global_authority account
     * raffle account and creator's nft ATA and global_authority's nft ATA
     * @param global_bump: global_authority's bump
     */
    pub fn withdraw_nft(ctx: Context<WithdrawNft>, global_bump: u8) -> Result<()> {
        let timestamp = Clock::get()?.unix_timestamp;
        let mut raffle = ctx.accounts.raffle.load_mut()?;
        let ts = timestamp;
        msg!("Timestamp: {}", ts);

        if timestamp < raffle.end_timestamp {
            return Err(RaffleError::RaffleNotEnded.into());
        }
        if raffle.creator != ctx.accounts.claimer.key() {
            return Err(RaffleError::NotCreator.into());
        }
        if raffle.count != 0 {
            return Err(RaffleError::OtherEntrants.into());
        }

        // Transfer NFT to the winner's wallet
        let src_token_account = &mut &ctx.accounts.src_nft_token_account;
        let dest_token_account = &mut &ctx.accounts.claimer_nft_token_account;
        let token_program = &mut &ctx.accounts.token_program;
        let seeds = &[GLOBAL_AUTHORITY_SEED.as_bytes(), &[global_bump]];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: src_token_account.to_account_info().clone(),
            to: dest_token_account.to_account_info().clone(),
            authority: ctx.accounts.global_authority.to_account_info(),
        };
        token::transfer(
            CpiContext::new_with_signer(
                token_program.clone().to_account_info(),
                cpi_accounts,
                signer,
            ),
            1,
        )?;
        raffle.whitelisted = 3;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init_if_needed,
        seeds = [GLOBAL_AUTHORITY_SEED.as_ref()],
        bump,
        payer = admin,
        space = 40
    )]
    pub global_authority: Account<'info, GlobalPool>,

    pub system_program: Program<'info, System>,
    // pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CreateRaffle<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    pub global_authority: Account<'info, GlobalPool>,

    #[account(zero)]
    pub raffle: AccountLoader<'info, RafflePool>,

    #[account(
        mut,
        constraint = owner_temp_nft_account.mint == *nft_mint_address.to_account_info().key,
        constraint = owner_temp_nft_account.owner == *admin.key,
    )]
    pub owner_temp_nft_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = dest_nft_token_account.mint == *nft_mint_address.to_account_info().key,
        constraint = dest_nft_token_account.owner == *global_authority.to_account_info().key,
    )]
    pub dest_nft_token_account: Account<'info, TokenAccount>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    pub nft_mint_address: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(global_bump: u8)]
pub struct UpdateRaffle<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut)]
    pub raffle: AccountLoader<'info, RafflePool>,
}

#[derive(Accounts)]
pub struct BuyTickets<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(mut)]
    pub raffle: AccountLoader<'info, RafflePool>,

    #[account(
        mut,
        seeds = [GLOBAL_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    pub global_authority: Account<'info, GlobalPool>,

    #[account(mut)]
    /// CHECK
    pub creator: AccountInfo<'info>,

    #[account(
        mut,
        constraint = creator_token_account.owner == creator.key(),
    )]
    pub creator_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = admin.key() == ADMIN_WALLET.parse::<Pubkey>().unwrap()
    )]
    pub admin: SystemAccount<'info>,

    #[account(
        mut,
        constraint = admin_token_account.owner == admin.key(),
    )]
    pub admin_token_account: Account<'info, TokenAccount>,


    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RevealWinner<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(mut)]
    pub raffle: AccountLoader<'info, RafflePool>,
}

#[derive(Accounts)]
#[instruction(global_bump: u8)]
pub struct ClaimReward<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_AUTHORITY_SEED.as_ref()],
        bump = global_bump,
    )]
    pub global_authority: Account<'info, GlobalPool>,

    #[account(mut)]
    pub raffle: AccountLoader<'info, RafflePool>,

    #[account(
        mut,
        constraint = claimer_nft_token_account.mint == *nft_mint_address.to_account_info().key,
        constraint = claimer_nft_token_account.owner == *claimer.key,
    )]
    pub claimer_nft_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = src_nft_token_account.mint == *nft_mint_address.to_account_info().key,
        constraint = src_nft_token_account.owner == *global_authority.to_account_info().key,
    )]
    pub src_nft_token_account: Account<'info, TokenAccount>,

    /// CHECK
    pub nft_mint_address: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(global_bump: u8)]
pub struct WithdrawNft<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_AUTHORITY_SEED.as_ref()],
        bump = global_bump,
    )]
    pub global_authority: Account<'info, GlobalPool>,

    #[account(mut)]
    pub raffle: AccountLoader<'info, RafflePool>,

    #[account(
        mut,
        constraint = claimer_nft_token_account.mint == *nft_mint_address.to_account_info().key,
        constraint = claimer_nft_token_account.owner == *claimer.key,
    )]
    pub claimer_nft_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = src_nft_token_account.mint == *nft_mint_address.to_account_info().key,
        constraint = src_nft_token_account.owner == *global_authority.to_account_info().key,
    )]
    pub src_nft_token_account: Account<'info, TokenAccount>,

    /// CHECK
    pub nft_mint_address: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}