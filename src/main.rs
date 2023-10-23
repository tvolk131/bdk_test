extern crate bdk;
extern crate bitcoin;

use std::str::FromStr;

use bdk::bitcoin::Network;
use bdk::blockchain::{Blockchain, ElectrumBlockchain};
use bdk::database::MemoryDatabase;
use bdk::electrum_client::Client;
use bdk::keys::bip39::Mnemonic;
use bdk::miniscript::descriptor::Wpkh;
use bdk::miniscript::Descriptor;
use bdk::miniscript::psbt::PsbtExt;
use bdk::wallet::{AddressIndex, Wallet};
use bdk::{SyncOptions, FeeRate, SignOptions};
use bitcoin::Address;
use bitcoin::bip32::{ExtendedPrivKey, ExtendedPubKey};
use bitcoin::psbt::PartiallySignedTransaction;
use bitcoin::secp256k1::Secp256k1;

fn main() {
    // 1. Generate a public key (we're hardcoding the mnemonic for simplicity, and so we can add some regtest funds to it)
    let mnemonic: Mnemonic = Mnemonic::from_entropy(&[
        64, 139, 40, 92, 18, 56, 54, 0, 79, 75, 136, 66, 199, 196, 131, 114, 222, 19, 130, 69, 12,
        13, 67, 154, 243, 69, 186, 127, 196, 154, 207, 112,
    ])
    .unwrap();
    let seed = mnemonic.to_seed("");
    println!("{:?}", mnemonic.word_iter().collect::<Vec<&str>>());
    println!("{:?}", seed);
    let secp = Secp256k1::new();

    // Generate the master private key
    let master_key = ExtendedPrivKey::new_master(Network::Testnet, &seed).unwrap();
    let master_pub = ExtendedPubKey::from_priv(&secp, &master_key);

    let descriptor = Descriptor::Wpkh(Wpkh::new(master_pub.public_key).unwrap());

    // Create the wallet
    let wallet = Wallet::new(
        &descriptor.to_string(),
        Some(&descriptor.to_string()),
        Network::Testnet,
        MemoryDatabase::new(),
    )
    .unwrap();

    let blockchain =
        ElectrumBlockchain::from(Client::new("ssl://electrum.blockstream.info:60002").unwrap());

    // Sync the wallet
    wallet.sync(&blockchain, SyncOptions::default()).unwrap();

    println!("Wallet balance: {}", wallet.get_balance().unwrap());

    let sender_public_key = wallet.get_address(AddressIndex::Peek(0)).unwrap().address;
    let receiver_public_key = Address::from_str("tb1qdwuyjaa5tcm0mrucla8vwk8fuluuat4az3dxre").unwrap().require_network(Network::Testnet).unwrap();
    println!("Sender Pubkey: {}", sender_public_key);
    println!("Receiver Pubkey: {:#?}", receiver_public_key);

    // 2. Create a PSBT that pays from the public key
    // For this example we'll pay to ourselves, but you can pay to any address
    let mut tx_builder = wallet.build_tx();
    tx_builder.add_recipient(receiver_public_key.script_pubkey(), 600).fee_rate(FeeRate::default_min_relay_fee());
    let (mut psbt, _details) = tx_builder.finish().unwrap();
    // _details.transaction.unwrap().input.first().unwrap().witness;
    println!("Inputs: {:#?}", psbt.inputs);

    // 3. Sign the PSBT (Assuming hardware wallet integration)
    // For this example, we'll assume that `sign_with_hardware_wallet` is a function you've implemented
    // that uses a hardware wallet to sign the PSBT.
    println!();
    println!("Unsigned PSBT: {}", psbt.serialize_hex());
    println!();
    for input in &psbt.inputs {
        println!("PSBT Input before signing: {:#?}", input);
        println!();
    }
    // psbt = sign_with_hardware_wallet(psbt, &wallet);
    // wallet.finalize_psbt(&mut psbt, SignOptions::default()).unwrap();
    let finalized = wallet.sign(&mut psbt, SignOptions::default()).unwrap();
    assert!(finalized);
    println!("Signed PSBT: {}", psbt.serialize_hex());
    println!();
    for input in &psbt.inputs {
        println!("PSBT Input after signing: {:#?}", input);
        println!();
    }

    // 4. Broadcast the signed transaction to the blockchain
    psbt.finalize_mut(&secp).expect("Failed to finalize PSBT");
    let tx = psbt.extract(&secp).unwrap();
    blockchain.broadcast(&tx).unwrap();

    // 5. Wait for the transaction to be confirmed
    // This is a simplified example; you may want to implement more robust logic.
    loop {
        wallet.sync(&blockchain, SyncOptions::default()).unwrap();
        if wallet
            .get_tx(&tx.txid(), true)
            .unwrap()
            .unwrap()
            .confirmation_time
            .is_some()
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}

// Dummy function to represent hardware wallet signing
// fn sign_with_hardware_wallet(mut psbt: PartiallySignedTransaction, wallet: &Wallet<MemoryDatabase>) -> PartiallySignedTransaction {
//     wallet.sign(&mut psbt, SignOptions::default()).unwrap();
//     psbt
// }
