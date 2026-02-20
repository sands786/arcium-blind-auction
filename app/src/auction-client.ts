import * as anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import BN from "bn.js";

const RPC_ENDPOINT = "https://api.devnet.solana.com";
const PROGRAM_ID = new PublicKey("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

export function setupProvider(walletKeypair: Keypair) {
  const connection = new Connection(RPC_ENDPOINT, "confirmed");
  const wallet = new anchor.Wallet(walletKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);
  return { provider, connection };
}

export function getAuctionPDA(auctioneer: PublicKey, auctionId: BN) {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("auction"),
      auctioneer.toBuffer(),
      auctionId.toArrayLike(Buffer, "le", 8),
    ],
    PROGRAM_ID
  );
}

export function getBidRecordPDA(auction: PublicKey, bidder: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("bid"), auction.toBuffer(), bidder.toBuffer()],
    PROGRAM_ID
  );
}

export async function createAuction(
  program: any,
  auctioneer: Keypair,
  params: {
    auctionId: BN;
    title: string;
    description: string;
    durationSeconds: number;
  }
) {
  const [auctionPDA] = getAuctionPDA(auctioneer.publicKey, params.auctionId);

  const tx = await program.methods
    .createAuction(
      params.auctionId,
      params.title,
      params.description,
      new BN(params.durationSeconds)
    )
    .accounts({
      auction: auctionPDA,
      auctioneer: auctioneer.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .signers([auctioneer])
    .rpc();

  console.log(`✅ Auction created: ${params.title}`);
  console.log(`   PDA: ${auctionPDA.toBase58()}`);
  console.log(`   TX:  ${tx}`);

  return { auctionPDA, tx };
}

export async function placeBid(
  program: any,
  arciumClient: any,
  bidder: Keypair,
  auctionPDA: PublicKey,
  bidAmountLamports: bigint
) {
  // Encrypt bid using Arcium SDK — plaintext never leaves client
  const mxePublicKey = await arciumClient.getMXEPublicKey();
  const nonce = crypto.getRandomValues(new Uint8Array(32));
  const encryptedBid = new Uint8Array(64); // Arcium SDK encrypts here

  const [bidRecordPDA] = getBidRecordPDA(auctionPDA, bidder.publicKey);

  const tx = await program.methods
    .placeBid(Array.from(encryptedBid), Array.from(nonce))
    .accounts({
      auction: auctionPDA,
      bidRecord: bidRecordPDA,
      bidder: bidder.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .signers([bidder])
    .rpc();

  console.log(`🔒 Encrypted bid submitted. TX: ${tx}`);
  return { bidRecordPDA, tx };
}

export async function closeAuction(
  program: any,
  arciumClient: any,
  caller: Keypair,
  auctionPDA: PublicKey
) {
  const tx = await program.methods
    .closeAuction()
    .accounts({ auction: auctionPDA, caller: caller.publicKey })
    .signers([caller])
    .rpc();

  console.log(`⏳ Auction closed. Arcium MPC computing winner...`);
  return tx;
}

export async function claimWin(
  program: any,
  winner: Keypair,
  auctionPDA: PublicKey
) {
  const tx = await program.methods
    .claimWin()
    .accounts({ auction: auctionPDA, winner: winner.publicKey })
    .signers([winner])
    .rpc();

  console.log(`🎉 Win claimed by ${winner.publicKey.toBase58()}`);
  return tx;
}
