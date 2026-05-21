use blowfish::Blowfish;
use cipher::generic_array::GenericArray;
use cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptError {
    #[error("invalid Blowfish key")]
    InvalidKey,
    #[error("encrypted input must contain an even number of hex digits")]
    OddHexLength,
    #[error("invalid hex data")]
    InvalidHex(#[from] hex::FromHexError),
}

pub fn encrypt_hex(key: &[u8], input: &str) -> Result<String, CryptError> {
    let cipher =
        Blowfish::<byteorder::BE>::new_from_slice(key).map_err(|_| CryptError::InvalidKey)?;
    let input = input.as_bytes();
    let padded_len = if input.len() % 8 == 0 {
        input.len()
    } else {
        input.len() + (8 - input.len() % 8)
    };
    let mut padded = vec![0_u8; padded_len];
    padded[..input.len()].copy_from_slice(input);

    for block in padded.chunks_exact_mut(8) {
        cipher.encrypt_block(GenericArray::from_mut_slice(block));
    }

    Ok(hex::encode(padded))
}

pub fn decrypt_hex(key: &[u8], input: &str) -> Result<Vec<u8>, CryptError> {
    if input.len() % 2 != 0 {
        return Err(CryptError::OddHexLength);
    }

    let cipher =
        Blowfish::<byteorder::BE>::new_from_slice(key).map_err(|_| CryptError::InvalidKey)?;
    let mut output = hex::decode(input)?;

    for block in output.chunks_exact_mut(8) {
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encryption_round_trips_with_zero_padding() {
        let encrypted = encrypt_hex(b"6#26FRL$ZWD", r#"{"hello":"world"}"#).unwrap();
        assert_eq!(
            encrypted,
            "ccc0a49a64193acb1750b799ab899a5dc10701e8f95a7cce"
        );
        let decrypted = decrypt_hex(b"6#26FRL$ZWD", &encrypted).unwrap();

        assert!(decrypted.starts_with(br#"{"hello":"world"}"#));
        assert_eq!(decrypted.len() % 8, 0);
    }
}
