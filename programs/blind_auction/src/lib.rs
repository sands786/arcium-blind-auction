use anchor_lang::prelude::*;
use arcium_anchor::{
    arcium_instruction,
    state::{Cluster, Mempool, MXEAccount, ExecutionPool},
    ID as ARCIUM_ID,
};

declare_id!("7TY1q4ZJA9juVLcb9dfKAtoiiUwsDsfD8szRFm2cVW4x");

#[program]
pub mod blind_auction {
    use super::*;

    pub fn create_auction(
        ctx: Context<CreateAuction>,
        auction_id: u64,
        title: String,
        description: String,
        duration_seconds: i64,
    ) -> Result<()> {
        require!(title.len() <= 64, AuctionError::TitleTooLong);
        require!(description.len() <= 256, AuctionError::DescriptionTooLong);
        require!(duration_seconds > 0, AuctionError::InvalidDuration);

        let auction = &mut ctx.accounts.auction;
        let clock = Clock::get()?;

        auction.auction_id = auction_id;
        auction.auctioneer = ctx.accounts.auctioneer.key();
        auction.title = title;
        auction.description = description;
        auction.start_time = clock.unix_timestamp;
        auction.end_time = clock.unix_timestamp + duration_seconds;
        auction.status = AuctionStatus::Open;
        auction.bid_count = 0;
        auction.winner = None;
        auction.bump = ctx.bumps.auction;

        emit!(AuctionCreated {
            auction_id,
            auctioneer: ctx.accounts.auctioneer.key(),
            end_time: auction.end_time,
        });

        msg!("Auction '{}' created.", auction.title);
        Ok(())
    }

    pub fn place_bid(
        ctx: Context<PlaceBid>,
        encrypted_bid: [u8; 64],
        bid_nonce: [u8; 32],
    ) -> Result<()> {
        let auction = &mut ctx.accounts.auction;
        let clock = Clock::get()?;

        require!(auction.status == AuctionStatus::Open, AuctionError::AuctionNotOpen);
        require!(clock.unix_timestamp < auction.end_time, AuctionError::AuctionExpired);

        let bid_record = &mut ctx.accounts.bid_record;
        bid_record.auction = auction.key();
        bid_record.bidder = ctx.accounts.bidder.key();
        bid_record.encrypted_bid = encrypted_bid;
        bid_record.bid_nonce = bid_nonce;
        bid_record.timestamp = clock.unix_timestamp;
        bid_record.bump = ctx.bumps.bid_record;

        auction.bid_count += 1;

        emit!(BidPlaced {
            auction_id: auction.auction_id,
            bidder: ctx.accounts.bidder.key(),
            bid_index: auction.bid_count,
        });

        msg!("Encrypted bid received. Total bids: {}", auction.bid_count);
        Ok(())
    }

    pub fn close_auction(ctx: Context<CloseAuction>) -> Result<()> {
        let auction = &mut ctx.accounts.auction;
        let clock = Clock::get()?;

        require!(auction.status == AuctionStatus::Open, AuctionError::AuctionNotOpen);
        require!(clock.unix_timestamp >= auction.end_time, AuctionError::AuctionStillActive);
        require!(auction.bid_count > 0, AuctionError::NoBids);

        auction.status = AuctionStatus::Computing;

        emit!(AuctionClosing { auction_id: auction.auction_id, bid_count: auction.bid_count });
        msg!("Auction submitted to Arcium MPC.");
        Ok(())
    }

    pub fn finalize_auction(
        ctx: Context<FinalizeAuction>,
        winner_pubkey: Pubkey,
        computation_proof: [u8; 32],
    ) -> Result<()> {
        let auction = &mut ctx.accounts.auction;

        require!(ctx.accounts.arcium_program.key() == ARCIUM_ID, AuctionError::UnauthorizedFinalizer);
        require!(auction.status == AuctionStatus::Computing, AuctionError::InvalidState);

        auction.status = AuctionStatus::Finalized;
        auction.winner = Some(winner_pubkey);
        auction.computation_proof = computation_proof;

        emit!(AuctionFinalized { auction_id: auction.auction_id, winner: winner_pubkey, computation_proof });
        msg!("Auction finalized. Winner: {}", winner_pubkey);
        Ok(())
    }

    pub fn claim_win(ctx: Context<ClaimWin>) -> Result<()> {
        let auction = &ctx.accounts.auction;
        require!(auction.status == AuctionStatus::Finalized, AuctionError::AuctionNotFinalized);
        require!(auction.winner == Some(ctx.accounts.winner.key()), AuctionError::NotTheWinner);
        emit!(WinClaimed { auction_id: auction.auction_id, winner: ctx.accounts.winner.key() });
        msg!("Winner claimed victory!");
        Ok(())
    }
}

#[account]
pub struct AuctionAccount {
    pub auction_id: u64,
    pub auctioneer: Pubkey,
    pub title: String,
    pub description: String,
    pub start_time: i64,
    pub end_time: i64,
    pub status: AuctionStatus,
    pub bid_count: u32,
    pub winner: Option<Pubkey>,
    pub computation_proof: [u8; 32],
    pub bump: u8,
}

impl AuctionAccount {
    pub const LEN: usize = 8 + 8 + 32 + (4+64) + (4+256) + 8 + 8 + 1 + 4 + (1+32) + 32 + 1;
}

#[account]
pub struct BidRecord {
    pub auction: Pubkey,
    pub bidder: Pubkey,
    pub encrypted_bid: [u8; 64],
    pub bid_nonce: [u8; 32],
    pub timestamp: i64,
    pub bump: u8,
}

impl BidRecord {
    pub const LEN: usize = 8 + 32 + 32 + 64 + 32 + 8 + 1;
}

#[derive(Accounts)]
#[instruction(auction_id: u64)]
pub struct CreateAuction<'info> {
    #[account(init, payer = auctioneer, space = AuctionAccount::LEN, seeds = [b"auction", auctioneer.key().as_ref(), &auction_id.to_le_bytes()], bump)]
    pub auction: Account<'info, AuctionAccount>,
    #[account(mut)]
    pub auctioneer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PlaceBid<'info> {
    #[account(mut)]
    pub auction: Account<'info, AuctionAccount>,
    #[account(init, payer = bidder, space = BidRecord::LEN, seeds = [b"bid", auction.key().as_ref(), bidder.key().as_ref()], bump)]
    pub bid_record: Account<'info, BidRecord>,
    #[account(mut)]
    pub bidder: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseAuction<'info> {
    #[account(mut)]
    pub auction: Account<'info, AuctionAccount>,
    #[account(mut)]
    pub caller: Signer<'info>,
    #[account(seeds = [b"mxe"], bump, seeds::program = ARCIUM_ID)]
    pub mxe_account: Account<'info, MXEAccount>,
    #[account(mut, seeds = [b"mempool", mxe_account.key().as_ref()], bump, seeds::program = ARCIUM_ID)]
    pub mempool: Account<'info, Mempool>,
    #[account(mut, seeds = [b"execpool", mxe_account.key().as_ref()], bump, seeds::program = ARCIUM_ID)]
    pub exec_pool: Account<'info, ExecutionPool>,
    #[account(seeds = [b"cluster", mxe_account.key().as_ref()], bump, seeds::program = ARCIUM_ID)]
    pub cluster: Account<'info, Cluster>,
    #[account(mut)]
    pub encrypted_ix: UncheckedAccount<'info>,
    #[account(address = ARCIUM_ID)]
    pub arcium_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FinalizeAuction<'info> {
    #[account(mut)]
    pub auction: Account<'info, AuctionAccount>,
    #[account(address = ARCIUM_ID)]
    pub arcium_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct ClaimWin<'info> {
    pub auction: Account<'info, AuctionAccount>,
    pub winner: Signer<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum AuctionStatus { Open, Computing, Finalized, Cancelled }

#[event]
pub struct AuctionCreated { pub auction_id: u64, pub auctioneer: Pubkey, pub end_time: i64 }
#[event]
pub struct BidPlaced { pub auction_id: u64, pub bidder: Pubkey, pub bid_index: u32 }
#[event]
pub struct AuctionClosing { pub auction_id: u64, pub bid_count: u32 }
#[event]
pub struct AuctionFinalized { pub auction_id: u64, pub winner: Pubkey, pub computation_proof: [u8; 32] }
#[event]
pub struct WinClaimed { pub auction_id: u64, pub winner: Pubkey }

#[error_code]
pub enum AuctionError {
    #[msg("Title too long")] TitleTooLong,
    #[msg("Description too long")] DescriptionTooLong,
    #[msg("Invalid duration")] InvalidDuration,
    #[msg("Auction not open")] AuctionNotOpen,
    #[msg("Auction expired")] AuctionExpired,
    #[msg("Already bid")] AlreadyBid,
    #[msg("Auction still active")] AuctionStillActive,
    #[msg("No bids placed")] NoBids,
    #[msg("Unauthorized finalizer")] UnauthorizedFinalizer,
    #[msg("Invalid state")] InvalidState,
    #[msg("Auction not finalized")] AuctionNotFinalized,
    #[msg("Not the winner")] NotTheWinner,
}
