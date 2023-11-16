use std::{
    collections::HashSet,
    fs::File,
    time::SystemTime, io::{BufReader, Read}
};

use bincode;
use k256::{sha2::{Digest, Sha256}, pkcs8::der::Writer};
use serde::{Deserialize, Serialize};

use super::transaction::{
    Sha256Hash,
    Transaction,
    TransactionValidityError,
    UTXOSet
};

use rand_core::OsRng;


#[derive(Debug, Deserialize, Serialize)]
pub struct Block {
    pub previous_block: Sha256Hash,
    pub time_stamp: SystemTime,
    tx_list: Vec<Transaction>,
    nonce: u64
}

#[derive(Debug)]
pub enum BlockValidityError {
    InvalidHash,
    InvalidTransaction,
    InvalidMinerReward
}

impl Block {
    pub fn new() -> Self {
        Block {
            previous_block: [0; 32],
            time_stamp: SystemTime::now(),
            tx_list: Vec::new(),
            nonce: 0
        }
    }

    pub fn from_file(file: &mut BufReader<File>) -> Option<Self> {
        let mut size = [0u8; 4];
        file.read_exact(&mut size).unwrap();
        let size = u32::from_ne_bytes(size);

        let mut buffer = vec![0; size as usize];
        let mut buffer = buffer.get_mut(..).unwrap();
        if let Err(_) = file.read_exact(&mut buffer) {
            return None;
        }

        file.seek_relative(4).unwrap();

        Some(bincode::deserialize(&buffer).unwrap())
    }

    pub fn from_file_backwads(file: &mut BufReader<File>) -> Option<Self> {
        let mut size = [0u8; 4];
        if let Err(_) = file.seek_relative(-4) {
            return None;
        }
        file.read_exact(&mut size).unwrap();
        let size = u32::from_ne_bytes(size);

        let mut buffer = vec![0; size as usize];
        let mut buffer = buffer.get_mut(..).unwrap();
        file.seek_relative(-4-(size as i64)).unwrap();
        file.read_exact(&mut buffer).unwrap();

        file.seek_relative(-4-(size as i64)).unwrap();

        Some(bincode::deserialize(&buffer).unwrap())
    }

    pub fn from_mempool(mempool: &HashSet<Transaction>, utxo_set: &UTXOSet)
            -> Self {

        let mut block = Block::new();
        let mut lowest_fee = u32::MAX;

        for tx in mempool {
            let fee = tx.is_valid(utxo_set).unwrap();
            if block.tx_list.len() < 5 {
                block.add(tx.clone());
                if fee < lowest_fee {
                    lowest_fee = fee;
                }
                continue;
            }

            if fee > lowest_fee {
                lowest_fee = block.remove_lowest_fee_transaction(utxo_set)
                    .unwrap();
                block.add(tx.clone());
                if fee < lowest_fee {
                    lowest_fee = fee;
                }
            }
        }

        block
    }

    pub fn set_previous_block(&mut self, previous: &Sha256Hash) {
        self.previous_block.copy_from_slice(previous);
    }

    pub fn add(&mut self, tx: Transaction) {
        self.tx_list.push(tx);
    }

    pub fn hash(&self) -> Sha256Hash {
        let serialized_block = bincode::serialize(self)
            .expect("Unable to serialize block");

        let mut hasher = Sha256::new();
        hasher.update(&serialized_block);
        hasher
            .finalize()
            .try_into()
            .expect("Wrong len")
    }

    pub fn is_valid_block(&self, difficulty: u32, reward: u32,
            utxo_set: &UTXOSet) -> Result<(), BlockValidityError>
    {
        let base = [0u8; 32];
        let hash = self.hash();
        if !are_first_n_bits_equal(&base, &hash, difficulty as usize) {
            return Err(BlockValidityError::InvalidHash);
        }

        let mut expected_miner_reward = reward;
        let mut actual_miner_reward = 0;
        for tx in &self.tx_list {
            match tx.is_valid(&utxo_set) {
                Ok(val) => expected_miner_reward += val,

                Err(err) => match err {
                    TransactionValidityError::InvalidOutputAmount(val) =>
                        actual_miner_reward += val,

                    _ => return Err(BlockValidityError::InvalidTransaction)
                }
            }
        }

        if expected_miner_reward != actual_miner_reward {
            return Err(BlockValidityError::InvalidMinerReward)
        }

        Ok(())
    }

    pub fn mine(&mut self, difficulty: u32) {
        let mut serialized_block = bincode::serialize(&self)
            .expect("Unable to serialize block");

        let base = [0u8; 32];

        let mut nonce = 0u64;
        let nonce_index_on_array = serialized_block.len() - 8 as usize;
        loop {
            let hash: Sha256Hash = Sha256::digest(&serialized_block)
                .try_into()
                .expect("Wrong len");

            if are_first_n_bits_equal(&base, &hash, difficulty as usize) {
                self.nonce = nonce;
                return;
            }

            nonce += 1;
            serialized_block[nonce_index_on_array..]
                .copy_from_slice(&nonce.to_le_bytes());
        }
    }

    pub fn update_utxo_set(&self, utxo_set: &mut UTXOSet) {
        for tx in &self.tx_list {
            for input in &tx.inputs {
                utxo_set.remove(&(input.core.tx_id, input.core.output_id));
            }
            for (i, output) in tx.outputs.iter().enumerate() {
                utxo_set.insert((tx.calculate_id(), i as u32), output.clone());
            }
        }
    }

    pub fn update_mempool(&self, mempool: &mut HashSet<Transaction>) {
        for tx in &self.tx_list {
            mempool.remove(&tx);
        }
    }

    pub fn rewind(&self, utxo_set: &mut UTXOSet,
            utxos_to_add: &mut HashSet<(Sha256Hash, u32)>)  {

        for tx in &self.tx_list {
            for i in 0..tx.outputs.len() {
                utxo_set.remove(&(tx.calculate_id(), i as u32));
                utxos_to_add.remove(&(tx.calculate_id(), i as u32));
            }
            for input in &tx.inputs {
                utxos_to_add.insert((input.core.tx_id, input.core.output_id));
            }
        }
    }

    pub fn write_to_file(&self, file: &mut File) {
        let serialized_block = bincode::serialize(self).unwrap();
        let len = serialized_block.len() as u32;

        file.write(&len.to_ne_bytes()).unwrap();
        file.write(&serialized_block).unwrap();
        file.write(&len.to_ne_bytes()).unwrap();
    }

    pub fn add_pending_utxos_to_utxo_set(&self,  utxo_set: &mut UTXOSet,
            utxos_to_add: &mut HashSet<(Sha256Hash, u32)>) {

        for tx in &self.tx_list {
            for (i, output) in tx.outputs.iter().enumerate() {
                if let Some(val) = utxos_to_add
                        .take(&(tx.calculate_id(), i as u32)) {

                    utxo_set.insert(val, output.clone());
                }
            }
        }
    }

    pub fn remove_lowest_fee_transaction(&mut self, utxo_set: &UTXOSet)
            -> Option<u32> {

        let mut lowest_fee_id: Option<u32> = None;
        let mut lowest_fee: Option<u32> = None;
        let mut second_lowest_fee: Option<u32> = None;
        for (i, tx) in self.tx_list.iter().enumerate() {
            let fee = tx.is_valid(utxo_set).unwrap();

            if let None = lowest_fee_id {
                lowest_fee_id = Some(i as u32);
                lowest_fee = Some(fee);
                continue;
            }

            if fee < lowest_fee.unwrap() {
                lowest_fee_id = Some(i as u32);
                second_lowest_fee = lowest_fee;
                lowest_fee = Some(fee);
            }
        }

        if let Some(i) = lowest_fee_id {
            self.tx_list.remove(i as usize);
        }

        second_lowest_fee
    }

    pub fn update_all_pending_utxos(chain: &mut BufReader<File>,
            utxo_set: &mut UTXOSet,
            utxos_to_add: &mut HashSet<(Sha256Hash, u32)>) {

        let mut bytes_rewinded = 0;

        while utxos_to_add.len() > 0 {
            let mut size = [0u8; 4];
            chain.seek_relative(-4).unwrap();
            chain.read_exact(&mut size).unwrap();
            let size: u32 = bincode::deserialize(&size).unwrap();
            bytes_rewinded += 8 + size;

            let block = Block::from_file_backwads(&mut *chain).unwrap();
            block.add_pending_utxos_to_utxo_set(&mut *utxo_set, &mut *utxos_to_add);
        }

        chain.seek_relative(bytes_rewinded as i64).unwrap();
    }
}

fn are_first_n_bits_equal(slice1: &[u8], slice2: &[u8], n: usize) -> bool {
    let full_bytes = n / 8;

    let remaining_bits = n % 8;

    if slice1.len() < full_bytes || slice2.len() < full_bytes {
        return false;
    }
    if slice1[..full_bytes] != slice2[..full_bytes] {
        return false;
    }

    if remaining_bits > 0 {
        let mask = (1u8 << remaining_bits) - 1;
        let last_byte1 = slice1[full_bytes] & mask;
        let last_byte2 = slice2[full_bytes] & mask;
        return last_byte1 == last_byte2;
    }

    true
}

