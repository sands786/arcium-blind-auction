use arcis_imports::*;

#[arcis_main]
pub fn find_winning_bid(
    mxe: &mut MXEContext,
    encrypted_bids: EncArray<Enc<u64>, 64>,
    bid_count: u32,
) -> u32 {
    let mut max_bid: Enc<u64> = encrypted_bids[0];
    let mut winner_idx: Enc<u32> = mxe.secret_share(0u32);

    for i in 1..bid_count as usize {
        let current_bid = encrypted_bids[i];
        let is_greater = current_bid.gt(&max_bid);
        max_bid = is_greater.select(current_bid, max_bid);
        winner_idx = is_greater.select(mxe.secret_share(i as u32), winner_idx);
    }

    winner_idx.reveal()
}
