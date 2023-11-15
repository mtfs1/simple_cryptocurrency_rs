use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    time::SystemTime
};

use bincode;
use k256::{
    ecdsa::{
        Signature, SigningKey, VerifyingKey,
        signature::{Signer, Verifier}
    },
    sha2::{Digest, Sha256}
};
use serde::{Deserialize, Serialize};


pub type Sha256Hash = [u8; 32];
pub type UTXOSet = HashMap<(Sha256Hash, u32), Output>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Output {
    to_pubkey: VerifyingKey,
    amount: u32
}

pub struct PartialOutput {
    to_pubkey: Option<VerifyingKey>,
    amount: Option<u32>
}

impl Output {
    pub fn new() -> PartialOutput {
        PartialOutput {
            to_pubkey: None,
            amount: None
        }
    }
}

impl PartialOutput {
    pub fn set_pubkey(mut self, key: VerifyingKey) -> Self {
        self.to_pubkey = Some(key);
        self
    }

    pub fn set_amount(mut self, amount: u32) -> Self {
        self.amount = Some(amount);
        self
    }

    pub fn collect(self) -> Output {
        Output {
            to_pubkey: self.to_pubkey
                .expect("Pubkey needs to be defined to collect"),
            amount: self.amount
                .expect("Amount needs to be defined to collect")
        }
    }
}


#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InputCore {
    pub tx_id: Sha256Hash,
    pub output_id: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Input {
    pub core: InputCore,
    signature: Signature
}

#[derive(Debug)]
pub struct PartialInput {
    tx_id: Option<Sha256Hash>,
    output_id: Option<u32>,
}

impl Input {
    pub fn new() -> PartialInput {
        PartialInput {
            tx_id: None,
            output_id: None
        }
    }

    pub fn verify(&self, pub_key: VerifyingKey) -> bool {
        let serialized_core = bincode::serialize(&self.core).unwrap();
        pub_key.verify(&serialized_core, &self.signature).is_ok()
    }
}

impl PartialInput {
    pub fn set_tx_id(mut self, id: &Sha256Hash) -> Self {
        self.tx_id = Some(*id);
        self
    }

    pub fn set_utxo_id(mut self, id: u32) -> Self {
        self.output_id = Some(id);
        self
    }

    pub fn sign(self, key: &SigningKey) -> Input {
        let core = InputCore {
            tx_id: self.tx_id
                .expect("Transaction id needs to be defined to sign"),
            output_id: self.output_id
                .expect("Output id needs to be defined to sign")
        };

        let serialized_core = bincode::serialize(&core).unwrap();
        let signature = key.sign(&serialized_core);

        Input {
            core,
            signature
        }
    }
}


#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Transaction {
    time_stamp: SystemTime,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>
}

#[derive(Debug)]
pub enum TransactionValidityError {
    InvalidOutputAmount(u32),
    InvalidSignature(u32),
    InputDoesNotExist(u32)
}

impl Transaction {
    pub fn new() -> Self {
        Transaction {
            time_stamp: SystemTime::now(),
            inputs: Vec::new(),
            outputs: Vec::new()
        }
    }

    pub fn add_input(&mut self, input: Input) {
        self.inputs.push(input);
    }

    pub fn add_output(&mut self, output: Output) {
        self.outputs.push(output);
    }

    pub fn calculate_id(&self) -> Sha256Hash {
        let serialized_tx = bincode::serialize(self).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(serialized_tx);
        hasher
            .finalize()
            .as_slice()
            .try_into()
            .expect("Wrong len")
    }

    pub fn is_valid(&self, utxo_set: &UTXOSet)
            -> Result<u32, TransactionValidityError> {

        let total_output = self.outputs
            .iter()
            .fold(0, |acc, val| acc + val.amount);

        let mut total_input = 0;
        for (i, input) in self.inputs.iter().enumerate() {
            let utxo = match utxo_set.get(
                    &(input.core.tx_id, input.core.output_id)) {
                Some(utxo) => utxo,
                None => return Err(
                    TransactionValidityError::InputDoesNotExist(i as u32)
                )
            };

            if !input.verify(utxo.to_pubkey) {
                return Err(
                    TransactionValidityError::InvalidSignature(i as u32)
                )
            }

            total_input += utxo.amount;
        }

        if total_output <= total_input {
            return Ok(total_input - total_output)
        }

        Err(TransactionValidityError::InvalidOutputAmount(
            total_output - total_input))
    }

    pub fn update_time(&mut self) {
        self.time_stamp = SystemTime::now();
    }
}

impl Hash for Transaction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let serialized_self =  bincode::serialize(self).unwrap();
        let hash = Sha256::digest(&serialized_self);
        state.write(&hash);
    }
}

