use hex;
use serde::{Deserialize, Serialize};
use serde_json;
use sha3::{Digest, Keccak256};

pub fn keccak(data: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    for item in data {
        hasher.update(item);
    }
    hasher.finalize().into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerkleLeaf {
    pub index: u32,
    pub recipient: String,
    pub amount: u64,
}

impl MerkleLeaf {
    pub fn parse_pubkey(&self) -> Result<[u8; 32], Box<dyn std::error::Error>> {
        let decoded = bs58::decode(&self.recipient).into_vec()?;
        if decoded.len() != 32 {
            return Err("Invalid Solana address length".into());
        }
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&decoded);
        Ok(pubkey)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerkleTree {
    pub root: String,
    pub tree: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeValue {
    pub leaf: MerkleLeaf,
    pub tree_index: usize,
}

impl MerkleTree {
    pub fn build_tree(leaves: Vec<MerkleLeaf>) -> Self {
        if leaves.is_empty() {
            panic!("Cannot build merkle tree with empty leaves");
        }

        let mut leaf_hashes: Vec<String> = leaves
            .iter()
            .map(|leaf| {
                let index_bytes = leaf.index.to_le_bytes();
                let recipient_pubkey = leaf.parse_pubkey().expect("Invalid Solana address");
                let amount_bytes = leaf.amount.to_le_bytes();
                let leaf_bytes: &[&[u8]] = &[&index_bytes, &recipient_pubkey, &amount_bytes];
                let mut leaf_hash = keccak(leaf_bytes);
                // Hash one more time to protect against the second pre-image attacks
                leaf_hash = keccak(&[&leaf_hash]);

                hex::encode(leaf_hash)
            })
            .collect();

        let mut tree = vec![leaf_hashes.clone()];

        while leaf_hashes.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in leaf_hashes.chunks(2) {
                let hash = if chunk.len() == 2 {
                    // Pair available - hash them in order (smaller first)
                    let hash1 = hex::decode(&chunk[0]).expect("Invalid hex");
                    let hash2 = hex::decode(&chunk[1]).expect("Invalid hex");
                    let mut hash1_array = [0u8; 32];
                    let mut hash2_array = [0u8; 32];
                    hash1_array.copy_from_slice(&hash1);
                    hash2_array.copy_from_slice(&hash2);

                    if hash1_array <= hash2_array {
                        hex::encode(keccak(&[&hash1_array, &hash2_array]))
                    } else {
                        hex::encode(keccak(&[&hash2_array, &hash1_array]))
                    }
                } else {
                    // Odd number - hash the last element with itself
                    let hash1 = hex::decode(&chunk[0]).expect("Invalid hex");
                    let mut hash1_array = [0u8; 32];
                    hash1_array.copy_from_slice(&hash1);
                    hex::encode(keccak(&[&hash1_array, &hash1_array]))
                };
                next_level.push(hash);
            }

            tree.push(next_level.clone());
            leaf_hashes = next_level;
        }

        let root = leaf_hashes[0].clone();

        MerkleTree { root, tree }
    }

    pub fn get_proof(&self, index: u32) -> Option<Vec<String>> {
        let index = index as usize;

        let num_leaves = if self.tree.is_empty() { 0 } else { self.tree[0].len() };
        if index >= num_leaves {
            return None;
        }

        let mut proof = Vec::new();
        let mut current_index = index;

        for level in 0..self.tree.len() - 1 {
            let current_level = &self.tree[level];

            let sibling_index = if current_index % 2 == 0 { current_index + 1 } else { current_index - 1 };

            if sibling_index < current_level.len() {
                proof.push(current_level[sibling_index].clone());
            }

            current_index /= 2;
        }

        Some(proof)
    }

    pub fn root_hex(&self) -> String {
        self.root.clone()
    }

    pub fn dump(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn load(data: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_leaves() -> Vec<MerkleLeaf> {
        vec![
            MerkleLeaf {
                index: 0,
                recipient: "8miSWoL8uhTZjA51YjJs6ddbi1oZYtNKwwgdpG2FmXp8".to_string(),
                amount: 100000000,
            },
            MerkleLeaf {
                index: 1,
                recipient: "9KGLQ4gqdCr5GfiHRNyNE3qwZD6N8AphE96dyxKKfURi".to_string(),
                amount: 100000000,
            },
            MerkleLeaf {
                index: 2,
                recipient: "EfjHTQfMTofQXkQpjndCFdnV8tpSfPTLJuo8tDAxWr9f".to_string(),
                amount: 100000000,
            },
            MerkleLeaf {
                index: 3,
                recipient: "FL7fsXqH4BvcCVWNXyujmpVbDjSu1StY2yWmUnVgSJSv".to_string(),
                amount: 100000000,
            },
        ]
    }

    fn verify_proof(leaf: &MerkleLeaf, merkle_root: &str, merkle_proof: Vec<String>) -> bool {
        let index_bytes = leaf.index.to_le_bytes();
        let recipient_pubkey = leaf.parse_pubkey().expect("Invalid Solana address");
        let amount_bytes = leaf.amount.to_le_bytes();
        let leaf_bytes: &[&[u8]] = &[&index_bytes, &recipient_pubkey, &amount_bytes];

        let mut leaf_hash = keccak(leaf_bytes);

        // Hash one more time to protect against the second pre-image attacks
        leaf_hash = keccak(&[&leaf_hash]);

        let mut computed_hash = leaf_hash;
        for proof_element_hex in merkle_proof.iter() {
            let proof_element = match hex::decode(proof_element_hex) {
                Ok(bytes) => {
                    if bytes.len() == 32 {
                        let mut array = [0u8; 32];
                        array.copy_from_slice(&bytes);
                        array
                    } else {
                        return false; // Invalid proof element
                    }
                }
                Err(_) => return false, // Invalid hex
            };

            if computed_hash <= proof_element {
                computed_hash = keccak(&[&computed_hash, &proof_element]);
            } else {
                computed_hash = keccak(&[&proof_element, &computed_hash]);
            }
        }

        let computed_root_hex = hex::encode(computed_hash);
        computed_root_hex == merkle_root
    }

    #[test]
    fn test_build_tree_with_four_leaves() {
        let leaves = create_test_leaves();
        let tree = MerkleTree::build_tree(leaves.clone());

        // println!("root: {:?}", tree.root_hex());
        // println!("proof: {:?}", tree.get_proof(1));
        // println!("tree: {:?}", tree.dump().unwrap());

        assert_eq!(tree.tree[0].len(), 4);
        assert_eq!(tree.tree.len(), 3); // leaf level + 2 intermediate levels
        assert_eq!(tree.tree[0].len(), 4); // leaf level
        assert_eq!(tree.tree[1].len(), 2); // intermediate level
        assert_eq!(tree.tree[2].len(), 1); // root level
        assert_eq!(tree.root, tree.tree[2][0]);
    }

    #[test]
    fn test_build_tree_with_single_leaf() {
        let leaves =
            vec![MerkleLeaf { index: 0, recipient: "11111111111111111111111111111112".to_string(), amount: 500 }];
        let tree = MerkleTree::build_tree(leaves.clone());

        assert_eq!(tree.tree.len(), 1); // only leaf level
        assert_eq!(tree.tree[0].len(), 1);
        assert_eq!(tree.root, tree.tree[0][0]);
    }

    #[test]
    fn test_build_tree_with_odd_number_of_leaves() {
        let mut leaves = create_test_leaves();
        leaves.push(MerkleLeaf { index: 4, recipient: "11111111111111111111111111111114".to_string(), amount: 500 });

        let tree = MerkleTree::build_tree(leaves.clone());

        assert_eq!(tree.tree[0].len(), 5); // leaf level
    }

    #[test]
    #[should_panic(expected = "Cannot build merkle tree with empty leaves")]
    fn test_build_tree_with_empty_leaves() {
        let leaves = vec![];
        MerkleTree::build_tree(leaves);
    }

    #[test]
    fn test_get_proof_valid_indices() {
        let leaves = create_test_leaves();
        let tree = MerkleTree::build_tree(leaves.clone());

        // Test proof for each leaf
        for i in 0..leaves.len() {
            let proof = tree.get_proof(i as u32);
            assert!(proof.is_some());

            let proof = proof.unwrap();
            assert!(verify_proof(&leaves[i], &tree.root, proof));
        }
    }

    #[test]
    fn test_get_proof_invalid_index() {
        let leaves = create_test_leaves();
        let tree = MerkleTree::build_tree(leaves.clone());

        // Test out of bounds index
        let proof = tree.get_proof(10);
        assert!(proof.is_none());
    }

    #[test]
    fn test_get_proof_single_leaf() {
        let leaves =
            vec![MerkleLeaf { index: 0, recipient: "11111111111111111111111111111112".to_string(), amount: 500 }];
        let tree = MerkleTree::build_tree(leaves.clone());

        let proof = tree.get_proof(0).unwrap();
        assert_eq!(proof.len(), 0); // No siblings needed for single leaf
        assert!(verify_proof(&leaves[0], &tree.root, proof));
    }

    #[test]
    fn test_merkle_proof_verification() {
        let leaves = create_test_leaves();
        let tree = MerkleTree::build_tree(leaves.clone());

        // Test that each leaf can be verified with its proof
        for (i, leaf) in leaves.iter().enumerate() {
            let proof = tree.get_proof(i as u32).unwrap();
            assert!(verify_proof(leaf, &tree.root, proof), "Failed to verify proof for leaf at index {}", i);
        }
    }

    #[test]
    fn test_merkle_proof_wrong_leaf() {
        let leaves = create_test_leaves();
        let tree = MerkleTree::build_tree(leaves.clone());

        // Get proof for index 0
        let proof = tree.get_proof(0).unwrap();

        // Try to verify with wrong leaf (index 1)
        let wrong_leaf = &leaves[1];
        assert!(!verify_proof(wrong_leaf, &tree.root, proof), "Should fail to verify wrong leaf with proof");
    }

    #[test]
    fn test_root_hex() {
        let leaves = create_test_leaves();
        let tree = MerkleTree::build_tree(leaves);

        let hex_root = tree.root_hex();
        assert_eq!(hex_root.len(), 64); // 32 bytes = 64 hex characters
        assert!(hex_root.chars().all(|c| c.is_ascii_hexdigit())); // All hex chars

        // Verify it's the same as the tree root (already hex encoded)
        assert_eq!(hex_root, tree.root);
    }

    #[test]
    fn test_proof_hex_format() {
        let leaves = create_test_leaves();
        let tree = MerkleTree::build_tree(leaves.clone());

        for (i, leaf) in leaves.iter().enumerate() {
            let proof = tree.get_proof(i as u32).unwrap();

            // Check that all proof elements are valid hex strings
            for proof_element in &proof {
                assert_eq!(proof_element.len(), 64); // 32 bytes = 64 hex characters
                assert!(proof_element.chars().all(|c| c.is_ascii_hexdigit())); // All hex chars

                // Verify we can decode it back to 32 bytes
                let decoded = hex::decode(proof_element).unwrap();
                assert_eq!(decoded.len(), 32);
            }

            // Verify the proof still works for verification
            assert!(verify_proof(leaf, &tree.root, proof));
        }
    }

    #[test]
    fn test_dump_and_load() {
        let leaves = create_test_leaves();
        let original_tree = MerkleTree::build_tree(leaves.clone());

        // Dump the tree
        let serialized = original_tree.dump().unwrap();

        // Load the tree back
        let loaded_tree = MerkleTree::load(&serialized).unwrap();

        // Verify the trees are identical
        assert_eq!(original_tree.root, loaded_tree.root);
        assert_eq!(original_tree.tree, loaded_tree.tree);
        assert_eq!(original_tree.get_proof(1), loaded_tree.get_proof(1));

        // Verify get_proof still works on the loaded tree
        for (i, leaf) in leaves.iter().enumerate() {
            let original_proof = original_tree.get_proof(i as u32).unwrap();
            let loaded_proof = loaded_tree.get_proof(i as u32).unwrap();

            assert_eq!(original_proof, loaded_proof);
            assert!(verify_proof(leaf, &loaded_tree.root, loaded_proof));
        }
    }
}
