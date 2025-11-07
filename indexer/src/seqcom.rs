// in src/seqcom.rs
use kaspa_hashes::Hash;
use blake2b_simd::Params;

pub fn merkle_hash(left: Hash, right: Hash) -> Hash {
    let mut hasher = Params::new().hash_length(32).to_state();
    hasher.update(&left.as_bytes());
    hasher.update(&right.as_bytes());
    let result = hasher.finalize();
    Hash::from_slice(result.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaspa_hashes::Hash;
    use std::str::FromStr;

    #[test]
    fn test_compute_seqcom() {
        let aidmr = Hash::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
        let parent_seqcom = Hash::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap();
        // Correct order: H(parent_seqcom || aidmr) per KIP-15
        let seqcom = merkle_hash(parent_seqcom, aidmr);
        // Correct expected hash for H(0x02 || 0x01)
        let expected_seqcom = Hash::from_str("2f7a6d7fcf39d35b702edf9b30bb304e0909c162a234a524503ef403d44b0ef3").unwrap();
        assert_eq!(seqcom, expected_seqcom);
    }

    #[test]
    fn test_compute_seqcom_chain() {
        let aidmr1 = Hash::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
        let parent_seqcom1 = Hash::from_str("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let seqcom1 = merkle_hash(aidmr1, parent_seqcom1);

        let aidmr2 = Hash::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap();
        let seqcom2 = merkle_hash(aidmr2, seqcom1);

        let aidmr3 = Hash::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap();
        let seqcom3 = merkle_hash(aidmr3, seqcom2);

        let expected_seqcom = Hash::from_str("080b5f27b2effa35a632c4efc0003e7e8e9170bc0baeb96283b292ced52f16b4").unwrap();
        assert_eq!(seqcom3, expected_seqcom);
    }
}
