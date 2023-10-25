const DB_MAGIC: &str = "bdk_wallet_electrum_example";
const SEND_AMOUNT: u64 = 600;
const STOP_GAP: usize = 50;
const BATCH_SIZE: usize = 5;

use std::str::FromStr;

use bdk::bitcoin::Network;
use bdk::chain::PersistBackend;
use bdk::keys::bip39::Mnemonic;
use bdk::miniscript::descriptor::Sh;
use bdk::miniscript::Descriptor;
use bdk::wallet::Update;
use bdk::{SignOptions, Wallet};
use bdk_electrum::electrum_client::{Client, ElectrumApi};
use bdk_electrum::ElectrumExt;
use bdk_electrum::ElectrumUpdate;
use bdk_file_store::Store;
use bitcoin::bip32::{ExtendedPrivKey, ExtendedPubKey};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::sighash::EcdsaSighashType;
use bitcoin::{PublicKey, Transaction};
use std::io::Write;

fn main() {
    // 1. Generate a public key (we're hardcoding the mnemonic for simplicity, and so we can add some testnet funds to it)
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
    let maker_master_key = ExtendedPrivKey::new_master(Network::Testnet, &seed).unwrap();
    let maker_master_pub = ExtendedPubKey::from_priv(&secp, &maker_master_key);

    // The public key of the maker.
    let maker_pubkey = PublicKey::new(maker_master_pub.public_key);

    // The public key of the taker.
    let taker_pubkey =
        PublicKey::from_str("028bde91b10013e08949a318018fedbd896534a549a278e220169ee2a36517c7aa")
            .unwrap();

    // The FROST public key belonging to the Fedimint federation.
    let federation_frost_pubkey =
        PublicKey::from_str("038f47dcd43ba6d97fc9ed2e3bba09b175a45fac55f0683e8cf771e8ced4572354")
            .unwrap();

    // Define the descriptor for the maker's wallet.
    let descriptor_str = format!("wpkh({}/84'/1'/0'/0/*)", maker_master_key);
    let maker_wallet_db_path = std::env::temp_dir().join("bdk-electrum-example-4");
    let maker_wallet_db =
        Store::<bdk::wallet::ChangeSet>::new_from_path(DB_MAGIC.as_bytes(), maker_wallet_db_path)
            .unwrap();
    // Create a wallet using the descriptor.
    let mut maker_wallet = Wallet::new(
        &descriptor_str,
        None,
        maker_wallet_db,
        Network::Testnet,
    )
    .unwrap();

    // Define the descriptor for the multisig setup.
    let multisig_escrow_descriptor = Descriptor::Sh(
        Sh::new_wsh_sortedmulti(2, vec![maker_pubkey, taker_pubkey, federation_frost_pubkey])
            .unwrap(),
    );
    let escrow_wallet_db_path = std::env::temp_dir().join("bdk-electrum-example-5");
    let escrow_wallet_db =
        Store::<bdk::wallet::ChangeSet>::new_from_path(DB_MAGIC.as_bytes(), escrow_wallet_db_path)
            .unwrap();
    // Create a wallet using the descriptor.
    let mut multisig_escrow_wallet = Wallet::new(
        &multisig_escrow_descriptor.to_string(),
        None,
        escrow_wallet_db,
        Network::Testnet,
    )
    .unwrap();

    // Sync the wallet
    println!("Syncing wallet...");
    sync_wallet(&mut maker_wallet).unwrap();
    sync_wallet(&mut multisig_escrow_wallet).unwrap();
    println!("Synced wallet!");

    // Generate a new address.
    let escrow_address = multisig_escrow_wallet.get_address(bdk::wallet::AddressIndex::New);
    println!("New escrow address: {}", escrow_address);

    // 2. Create a PSBT that pays from the public key
    let mut psbt = {
        let mut builder = maker_wallet.build_tx();
        builder.sighash(EcdsaSighashType::All.into());
        builder.add_recipient(escrow_address.script_pubkey(), SEND_AMOUNT);
        builder.finish().unwrap()
    };
    println!("PSBT Before Signing: {:#?}", psbt);
    let finalized = maker_wallet
        .sign(&mut psbt, SignOptions::default())
        .unwrap();
    println!("PSBT After Signing: {:#?}", psbt);
    assert!(finalized);

    // Check the wallet balance.;
    println!("Maker Wallet Balance: {}", maker_wallet.get_balance());
    println!(
        "Multisig Escrow Wallet Balance: {}",
        multisig_escrow_wallet.get_balance()
    );
}

fn sync_wallet<T: PersistBackend<bdk::wallet::ChangeSet>>(
    wallet: &mut Wallet<T>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new("ssl://electrum.blockstream.info:60002")?;

    let prev_tip = wallet.latest_checkpoint();
    let keychain_spks = wallet
        .spks_of_all_keychains()
        .into_iter()
        .map(|(k, k_spks)| {
            let mut once = Some(());
            let mut stdout = std::io::stdout();
            let k_spks = k_spks
                .inspect(move |(spk_i, _)| match once.take() {
                    Some(_) => print!("Scanning keychain [{:?}]", k),
                    None => print!(" {:<3}", spk_i),
                })
                .inspect(move |_| stdout.flush().expect("must flush"));
            (k, k_spks)
        })
        .collect();

    let (
        ElectrumUpdate {
            chain_update,
            relevant_txids,
        },
        keychain_update,
    ) = client.scan(prev_tip, keychain_spks, None, None, STOP_GAP, BATCH_SIZE)?;

    println!();

    let missing = relevant_txids.missing_full_txs(wallet.as_ref());
    let graph_update = relevant_txids.into_confirmation_time_tx_graph(&client, None, missing)?;

    let wallet_update = Update {
        last_active_indices: keychain_update,
        graph: graph_update,
        chain: Some(chain_update),
    };
    wallet.apply_update(wallet_update)?;
    // TODO - Remove this unwrap.
    wallet.commit().unwrap();

    Ok(())
}
