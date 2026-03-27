use ed25519_dalek::*;
use hex;

pub fn sign_data(private_key: &SigningKey, data: &[u8]) -> String {
    let signature: Signature = private_key.sign(data);
    hex::encode(signature.to_bytes())
}

pub fn verify_signature(public_key: &str, signature_hex: &str, data: &[u8]) -> bool {
    let Ok(pub_bytes) = hex::decode(public_key) else {
        return false;
    };
    let Ok(pub_array) = pub_bytes.as_slice().try_into() else {
        return false;
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&pub_array) else {
        return false;
    };

    let Ok(sig_bytes) = hex::decode(signature_hex) else {
        return false;
    };
    let Ok(sig_array) = sig_bytes.as_slice().try_into() else {
        return false;
    };

    let signature = Signature::from_bytes(&sig_array);

    verifying_key.verify_strict(data, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_sign_and_verify() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());

        let data = b"some message";

        let signature_hex = sign_data(&signing_key, data);

        let result = verify_signature(&public_key_hex, &signature_hex, data);

        assert!(result, "the signature must be valid");
    }
}
