use aes::Aes128;
use aes_gcm::{
    Aes128Gcm, Nonce,
    aead::{AeadInPlace, KeyInit as GcmKeyInit},
};
use base64::{
    Engine as _,
    alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig, general_purpose::STANDARD as BASE64},
};

const BASE64_LENIENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_allow_trailing_bits(true)
        .with_decode_padding_mode(DecodePaddingMode::Indifferent),
);
use ecb::cipher::{BlockDecryptMut, BlockEncryptMut, block_padding::Pkcs7};

pub const GENERIC_ECB_KEY: &str = "a3K8Bx%2r8Y7#xDh";
pub const GENERIC_GCM_KEY: &str = "{yxAHAY_Lm6pbC/<";
pub const GCM_IV: &[u8; 12] = b"\x54\x40\x78\x44\x49\x67\x5a\x51\x6c\x5e\x63\x13";
pub const GCM_AAD: &[u8] = b"qualcomm-test";

type Aes128EcbEnc = ecb::Encryptor<Aes128>;
type Aes128EcbDec = ecb::Decryptor<Aes128>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EncryptionMode {
    Ecb,
    Gcm,
}

impl std::fmt::Display for EncryptionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncryptionMode::Ecb => write!(f, "AES-128-ECB"),
            EncryptionMode::Gcm => write!(f, "AES-128-GCM"),
        }
    }
}

pub struct EncryptedPack {
    pub pack: String,
    pub tag: Option<String>,
}

pub fn encrypt_ecb(plaintext: &str, key: &str) -> Result<String, String> {
    let key_bytes = key.as_bytes();
    if key_bytes.len() != 16 {
        return Err(format!("Invalid AES-128 key length: {}", key_bytes.len()));
    }
    let pt = plaintext.as_bytes();
    let mut buf = vec![0u8; pt.len() + 16];
    buf[..pt.len()].copy_from_slice(pt);

    let ciphertext = Aes128EcbEnc::new_from_slice(key_bytes)
        .map_err(|e| format!("ECB init error: {e}"))?
        .encrypt_padded_mut::<Pkcs7>(&mut buf, pt.len())
        .map_err(|e| format!("ECB encrypt error: {e}"))?;

    Ok(BASE64.encode(ciphertext))
}

pub fn decrypt_ecb(pack_b64: &str, key: &str) -> Result<String, String> {
    let key_bytes = key.as_bytes();
    if key_bytes.len() != 16 {
        return Err(format!("Invalid AES-128 key length: {}", key_bytes.len()));
    }
    let mut buf = BASE64_LENIENT
        .decode(pack_b64.trim())
        .map_err(|e| format!("Base64 decode error: {e}"))?;

    let decrypted = Aes128EcbDec::new_from_slice(key_bytes)
        .map_err(|e| format!("ECB init error: {e}"))?
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| format!("ECB decrypt error: {e}"))?;

    // Trim any trailing non-JSON bytes just like PythonCLI/gree.py does
    let slice = match decrypted.iter().rposition(|&b| b == b'}') {
        Some(pos) => &decrypted[..=pos],
        None => decrypted,
    };
    String::from_utf8(slice.to_vec()).map_err(|e| format!("UTF-8 decode error: {e}"))
}

pub fn encrypt_gcm(plaintext: &str, key: &str) -> Result<EncryptedPack, String> {
    let key_bytes = key.as_bytes();
    if key_bytes.len() != 16 {
        return Err(format!("Invalid AES-128 key length: {}", key_bytes.len()));
    }
    let cipher = Aes128Gcm::new_from_slice(key_bytes)
        .map_err(|e| format!("GCM init error: {e}"))?;
    let nonce = Nonce::from_slice(GCM_IV);
    let mut buf = plaintext.as_bytes().to_vec();

    let tag = cipher
        .encrypt_in_place_detached(nonce, GCM_AAD, &mut buf)
        .map_err(|e| format!("GCM encrypt error: {e}"))?;

    Ok(EncryptedPack {
        pack: BASE64.encode(&buf),
        tag: Some(BASE64.encode(tag.as_slice())),
    })
}

pub fn decrypt_gcm(pack_b64: &str, tag_b64: &str, key: &str) -> Result<String, String> {
    let key_bytes = key.as_bytes();
    if key_bytes.len() != 16 {
        return Err(format!("Invalid AES-128 key length: {}", key_bytes.len()));
    }
    let cipher = Aes128Gcm::new_from_slice(key_bytes)
        .map_err(|e| format!("GCM init error: {e}"))?;
    let nonce = Nonce::from_slice(GCM_IV);
    let mut buf = BASE64_LENIENT
        .decode(pack_b64.trim())
        .map_err(|e| format!("Base64 pack decode error: {e}"))?;
    let tag_bytes = BASE64_LENIENT
        .decode(tag_b64.trim())
        .map_err(|e| format!("Base64 tag decode error: {e}"))?;
    if tag_bytes.len() != 16 {
        return Err(format!("Invalid GCM tag length: {}", tag_bytes.len()));
    }
    let tag = aes_gcm::Tag::from_slice(&tag_bytes);

    cipher
        .decrypt_in_place_detached(nonce, GCM_AAD, &mut buf, tag)
        .map_err(|e| format!("GCM decrypt/verify error: {e}"))?;

    // Some Gree firmwares pad with 0xFF bytes in GCM responses
    buf.retain(|&b| b != 0xff);
    String::from_utf8(buf).map_err(|e| format!("UTF-8 decode error: {e}"))
}

pub fn encrypt_pack(plaintext: &str, key: &str, mode: EncryptionMode) -> Result<EncryptedPack, String> {
    match mode {
        EncryptionMode::Ecb => Ok(EncryptedPack {
            pack: encrypt_ecb(plaintext, key)?,
            tag: None,
        }),
        EncryptionMode::Gcm => encrypt_gcm(plaintext, key),
    }
}

pub fn decrypt_pack(
    pack_b64: &str,
    tag_b64: Option<&str>,
    key: &str,
    mode: EncryptionMode,
) -> Result<String, String> {
    match mode {
        EncryptionMode::Ecb => decrypt_ecb(pack_b64, key),
        EncryptionMode::Gcm => {
            let tag = tag_b64.ok_or_else(|| "Missing GCM tag in response".to_string())?;
            decrypt_gcm(pack_b64, tag, key)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecb_roundtrip() {
        let msg = r#"{"mac":"001122334455","t":"bind","uid":0}"#;
        let enc = encrypt_ecb(msg, GENERIC_ECB_KEY).unwrap();
        let dec = decrypt_ecb(&enc, GENERIC_ECB_KEY).unwrap();
        assert_eq!(dec, msg);
    }

    #[test]
    fn gcm_roundtrip() {
        let msg = r#"{"mac":"001122334455","t":"bind","uid":0}"#;
        let enc = encrypt_gcm(msg, GENERIC_GCM_KEY).unwrap();
        let dec = decrypt_gcm(&enc.pack, enc.tag.as_deref().unwrap(), GENERIC_GCM_KEY).unwrap();
        assert_eq!(dec, msg);
    }

    #[test]
    fn decrypt_lenient_base64_and_json_trim() {
        let sample = r#"{"t":"dev","cid":"001122334455","brand":"gree","ver":"V2.0.0"}"#;
        let enc = encrypt_ecb(sample, GENERIC_ECB_KEY).unwrap();
        let dec = decrypt_ecb(&format!("  {enc}  "), GENERIC_ECB_KEY).unwrap();
        assert_eq!(dec, sample);
    }
}
