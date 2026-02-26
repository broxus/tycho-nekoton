use ed25519_dalek::Signer;
use sha2::{Digest, Sha256};
use std::borrow::Cow;

#[derive(Debug, Default, Clone, Copy)]
pub enum SignatureContext {
    #[default]
    Empty,
    SignatureId(i32),
    SignatureDomain(i32),
}

impl SignatureContext {
    pub fn sign(&self, key: &ed25519_dalek::SigningKey, data: &[u8]) -> ed25519_dalek::Signature {
        let data = match self {
            SignatureContext::SignatureId(global_id) => {
                let extended_data = Self::extend_with_signature_id(data, *global_id);
                Cow::Owned(extended_data)
            }
            SignatureContext::SignatureDomain(global_id) => {
                let extended_data = Self::extend_with_signature_domain(data, *global_id);
                Cow::Owned(extended_data)
            }
            _ => Cow::Borrowed(data),
        };

        key.sign(&data)
    }

    fn extend_with_signature_id(data: &[u8], global_id: i32) -> Vec<u8> {
        let mut extended_data = Vec::with_capacity(4 + data.len());
        extended_data.extend_from_slice(&global_id.to_be_bytes());
        extended_data.extend_from_slice(data);
        extended_data
    }

    fn extend_with_signature_domain(data: &[u8], global_id: i32) -> Vec<u8> {
        let hash = Self::l2_hash(global_id);
        let mut result = Vec::with_capacity(32 + data.len());
        result.extend_from_slice(&hash);
        result.extend_from_slice(data);
        result
    }

    fn l2_hash(global_id: i32) -> Vec<u8> {
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&0x71b34ee1u32.to_le_bytes());
        data.extend_from_slice(&global_id.to_le_bytes());
        Sha256::digest(data).to_vec()
    }
}

#[derive(Debug)]
pub struct ToVerify {
    pub ctx: SignatureContext,
    pub data: Vec<u8>,
}

impl ToVerify {
    pub fn prepare(&self) -> Vec<u8> {
        let mut output = Vec::new();
        match self.ctx {
            SignatureContext::SignatureId(global_id) => {
                output.extend_from_slice(&global_id.to_be_bytes());
            }
            SignatureContext::SignatureDomain(global_id) => {
                let hash = SignatureContext::l2_hash(global_id);
                output.extend_from_slice(&hash);
            }
            _ => {}
        }
        output.extend_from_slice(&self.data);
        output
    }
}
