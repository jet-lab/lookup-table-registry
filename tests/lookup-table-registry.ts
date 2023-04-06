import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { publicKey } from "@project-serum/anchor/dist/cjs/utils";
import { TOKEN_PROGRAM_ID } from "@project-serum/anchor/dist/cjs/utils/token";
import { AddressLookupTableProgram, PublicKey, SystemProgram } from "@solana/web3.js";
import { LookupTableRegistry } from "../target/types/lookup_table_registry";

describe("lookup-table-registry", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());
  const provider = anchor.getProvider();

  const program = anchor.workspace.LookupTableRegistry as Program<LookupTableRegistry>;

  it("Initialises a table", async () => {
    const [registryAccount, _] = publicKey.findProgramAddressSync([provider.publicKey.toBytes()], program.programId)
    const tx = await program.methods.initRegistryAccount().accounts({
      authority: provider.publicKey,
      payer: provider.publicKey,
      registryAccount,
    }).rpc();
    console.log("Your transaction signature", tx);
  });

  let recentSlot = 0;
  let lookupTableAddress: PublicKey;

  it("Adds a lookup account to the table", async () => {
    const [registryAccount, _] = publicKey.findProgramAddressSync([provider.publicKey.toBytes()], program.programId)
    recentSlot = await provider.connection.getSlot();
    const [_ix, lookupTable] = AddressLookupTableProgram.createLookupTable({
      authority: provider.publicKey,
      payer: provider.publicKey,
      recentSlot
    });
    lookupTableAddress = lookupTable;
    const tx = await program.methods.createLookupTable(new anchor.BN(recentSlot), new anchor.BN(1)).accounts({
      authority: provider.publicKey,
      payer: provider.publicKey,
      registryAccount,
      lookupTable,
      addressLookupTableProgram: AddressLookupTableProgram.programId,
    }).rpc({
      skipPreflight: true,
    });
    console.log("Your transaction signature", tx);
  })
  it("Appends accounts to a lookup table", async () => {
    const [registryAccount, _] = publicKey.findProgramAddressSync([provider.publicKey.toBytes()], program.programId);
    const newAddresses = [
      program.programId,
      TOKEN_PROGRAM_ID,
      provider.publicKey,
    ];
    const tx = await program.methods.appendToLookupTable(new anchor.BN(1), newAddresses).accounts({
      authority: provider.publicKey,
      payer: provider.publicKey,
      registryAccount,
      lookupTable: lookupTableAddress,
      addressLookupTableProgram: AddressLookupTableProgram.programId,
    }).rpc({
      skipPreflight: true,
    });
    console.log("Your transaction signature", tx);
  });
});
