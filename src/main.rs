const DB_MAGIC: &str = "bdk_wallet_electrum_example";
const SEND_AMOUNT: u64 = 5000;
const STOP_GAP: usize = 50;
const BATCH_SIZE: usize = 5;

use std::str::FromStr;

use bdk::bitcoin::Network;
use bdk::miniscript::descriptor::Sh;
use bdk::miniscript::Descriptor;
use bdk::Wallet;
use bdk_file_store::Store;
use bitcoin::PublicKey;

fn main() {
    // The public key of the maker.
    let maker_pubkey =
        PublicKey::from_str("032b8324c93575034047a52e9bca05a46d8347046b91a032eff07d5de8d3f2730b")
            .unwrap();

    // The public key of the taker.
    let taker_pubkey =
        PublicKey::from_str("028bde91b10013e08949a318018fedbd896534a549a278e220169ee2a36517c7aa")
            .unwrap();

    // The FROST public key belonging to the Fedimint federation.
    let federation_frost_pubkey =
        PublicKey::from_str("038f47dcd43ba6d97fc9ed2e3bba09b175a45fac55f0683e8cf771e8ced4572354")
            .unwrap();

    // Define the descriptor for the multisig setup.
    let descriptor = Descriptor::Sh(
        Sh::new_wsh_sortedmulti(2, vec![maker_pubkey, taker_pubkey, federation_frost_pubkey])
            .unwrap(),
    );

    let db_path = std::env::temp_dir().join("bdk-electrum-example-2");
    let db = Store::<bdk::wallet::ChangeSet>::new_from_path(DB_MAGIC.as_bytes(), db_path).unwrap();

    // Create a wallet using the descriptor.
    let mut wallet = Wallet::new(&descriptor.to_string(), None, db, Network::Testnet).unwrap();

    // Generate a new address.
    let address = wallet.get_address(bdk::wallet::AddressIndex::New);
    println!("New address: {}", address);

    // Sync the wallet with the blockchain.
    // wallet.sync(noop_progress(), None).unwrap();

    // Check the wallet balance.
    let balance = wallet.get_balance();
    println!("Wallet balance: {}", balance);
}
