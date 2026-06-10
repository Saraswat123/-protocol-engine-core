use crate::attestation::AttestationPool;
use std::collections::HashMap;

/// Simplified LMD-GHOST fork choice.
/// Tracks a DAG of block_root → parent_root and votes per root.
pub struct ForkChoice {
    /// block_root → parent_root
    pub parents: HashMap<[u8; 32], [u8; 32]>,
    pub pool: AttestationPool,
    pub justified_root: [u8; 32],
}

impl ForkChoice {
    pub fn new(genesis_root: [u8; 32]) -> Self {
        let mut parents = HashMap::new();
        parents.insert(genesis_root, [0u8; 32]);
        Self {
            parents,
            pool: AttestationPool::new(),
            justified_root: genesis_root,
        }
    }

    pub fn on_block(&mut self, root: [u8; 32], parent: [u8; 32]) {
        self.parents.insert(root, parent);
    }

    /// LMD-GHOST: starting from justified root, always pick child with most votes.
    pub fn head(&self) -> [u8; 32] {
        let children = self.build_children_map();
        let mut current = self.justified_root;

        loop {
            let kids = match children.get(&current) {
                Some(v) if !v.is_empty() => v,
                _ => break,
            };
            // pick child with highest vote weight; tie-break by lexicographic order
            let best = kids
                .iter()
                .max_by_key(|r| (self.pool.vote_weight(r), *r))
                .unwrap();
            current = *best;
        }
        current
    }

    fn build_children_map(&self) -> HashMap<[u8; 32], Vec<[u8; 32]>> {
        let mut map: HashMap<[u8; 32], Vec<[u8; 32]>> = HashMap::new();
        for (child, parent) in &self.parents {
            map.entry(*parent).or_default().push(*child);
        }
        map
    }
}
