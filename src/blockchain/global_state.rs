use std::{
    collections::HashSet,
    fs::{File, OpenOptions, self},
    io::{Seek, SeekFrom, Write},
    sync::Mutex
};

use serde::{Serialize, Deserialize};

use super::transaction::{Sha256Hash, Transaction, UTXOSet};


pub struct StateWithFile<T>
    where T: Serialize + for <'a> Deserialize<'a>
{
    file: File,
    state: T
}

impl<T> StateWithFile<T>
    where T: Serialize + for <'a> Deserialize<'a>
{
    pub fn new(file: &str, state: T) -> Self {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file)
            .unwrap();

        file.seek(SeekFrom::Start(0)).unwrap();

        let mut state = state;
        if let Ok(val) = bincode::deserialize_from(&mut file) {
            state = val;
        } else {
            let serialized_state = bincode::serialize(&state).unwrap();
            file.write_all(&serialized_state).unwrap();
        }

        StateWithFile {
            file,
            state
        }
    }

    pub fn set_state(&mut self, new_state: T) {
        self.state = new_state;
        self.update();
    }

    pub fn update(&mut self) {
        self.file.seek(SeekFrom::Start(0)).unwrap();
        self.file.set_len(0).unwrap();
        let serialized_state = bincode::serialize(&self.state).unwrap();
        self.file.write_all(&serialized_state).unwrap();
    }
}

impl<T> std::ops::Deref for StateWithFile<T>
    where T: Serialize + for <'a> Deserialize<'a>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<T> std::ops::DerefMut for StateWithFile<T>
    where T: Serialize + for <'a> Deserialize<'a>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}


pub struct GlobalState {
    pub block_height: Mutex<StateWithFile<u32>>,
    pub chain: Mutex<File>,
    pub utxo_set: Mutex<StateWithFile<UTXOSet>>,
    pub mempool:  Mutex<StateWithFile<HashSet<Transaction>>>,
    pub difficulty: Mutex<StateWithFile<u32>>,
    pub reward: Mutex<StateWithFile<u32>>,
    pub previous_block_hash: Mutex<StateWithFile<Sha256Hash>>
}

impl GlobalState {
    pub fn new() -> Self {
        fs::create_dir_all("./.state").unwrap();

        let block_height = StateWithFile::new("./.state/block_height", 0);
        println!("[BLOCK HEIGHT][{}]", *block_height);
        let block_height = Mutex::new(block_height);

        let chain = Mutex::new(OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open("./.state/chain")
            .unwrap()
        );
        let utxo_set = UTXOSet::new();
        let utxo_set = StateWithFile::new("./.state/utxo_set", utxo_set);
        println!("[UTXO SET][{}]", utxo_set.len());
        let utxo_set = Mutex::new(utxo_set);

        let mempool = HashSet::<Transaction>::new();
        let mempool = StateWithFile::new("./.state/mempool", mempool);
        println!("[MEMPOOL][{}]", mempool.len());
        let mempool = Mutex::new(mempool);

        let difficulty = StateWithFile::new("./.state/difficulty", 20);
        println!("[DIFFICULTY][{}]", *difficulty);
        let difficulty = Mutex::new(difficulty);

        let reward = StateWithFile::new("./.state/reward", 10);
        println!("[REWARD][{}]", *reward);
        let reward = Mutex::new(reward);

        let previous_block_hash = StateWithFile::new("./.state/previous_hash",
            [0u8; 32]);
        let previous_block_hash = Mutex::new(previous_block_hash);

        GlobalState {
            block_height,
            chain,
            utxo_set,
            mempool,
            difficulty,
            reward,
            previous_block_hash
        }
    }
}

