use bitvec::{access::BitSafeU8, order::Lsb0, vec::BitVec};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, VecDeque},
};

use crate::error::CoalescedError;

#[derive(Debug)]
enum HuffmanTree {
    Node(Box<HuffmanTree>, Box<HuffmanTree>),
    Leaf(char, u32),
}

impl HuffmanTree {
    fn frequency(&self) -> u32 {
        match *self {
            HuffmanTree::Node(ref left, ref right) => left.frequency() + right.frequency(),
            HuffmanTree::Leaf(_, freq) => freq,
        }
    }
}

impl PartialEq for HuffmanTree {
    fn eq(&self, other: &Self) -> bool {
        self.frequency().eq(&other.frequency())
    }
}

impl Eq for HuffmanTree {}

impl Ord for HuffmanTree {
    fn cmp(&self, other: &Self) -> Ordering {
        self.frequency().cmp(&other.frequency()).reverse()
    }
}

impl PartialOrd for HuffmanTree {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Map containing character frequencies to build a huffman tree from
#[derive(Default)]
pub struct FrequencyMap(pub HashMap<char, u32>);

impl FrequencyMap {
    /// Updates the frequency map based on the characters
    /// present in the provided string
    pub fn push_str(&mut self, value: &str) {
        for c in value.chars() {
            self.push(c)
        }
    }

    /// Updates the frequency map for the provided character
    #[inline]
    pub fn push(&mut self, value: char) {
        *self.0.entry(value).or_insert(0) += 1;
    }
}

pub struct Huffman {
    /// Mapping from chars to their huffman encoded bits
    mapping: HashMap<char, BitVec>,
    /// Flattened pairs from the huffman tree
    pairs: Vec<(i32, i32)>,
}

impl Huffman {
    pub fn new(freq: FrequencyMap) -> Self {
        let huffman_tree = Self::build_tree(freq);
        let mut huffman_mapping = HashMap::new();
        Self::generate_huffman_codes(&huffman_tree, BitVec::new(), &mut huffman_mapping);
        let pairs = Self::collect_pairs(&huffman_tree);

        Self {
            mapping: huffman_mapping,
            pairs,
        }
    }

    pub fn get_pairs(&self) -> &[(i32, i32)] {
        &self.pairs
    }

    // Encode the input text
    pub fn encode(&self, text: &str, output: &mut BitVec<BitSafeU8, Lsb0>) {
        for character in text.chars() {
            if let Some(code) = self.mapping.get(&character) {
                output.extend(code);
            }
        }
    }

    pub fn decode(
        compressed_data: &[u8],
        pairs: &[(i32, i32)],
        position: usize,
        max_length: usize,
    ) -> Result<String, CoalescedError> {
        let mut sb = String::new();
        let mut cur_node = pairs.len() - 1;
        let end = compressed_data.len() * 8;

        let mut pos = position;

        while pos < end && sb.len() < max_length {
            let sample = compressed_data[pos / 8] & (1 << (pos % 8));
            let next = pairs[cur_node];
            let next = if sample != 0 { next.1 } else { next.0 };

            if next < 0 {
                let ch = (-1 - next) as u16;
                if ch == 0 {
                    break;
                }
                sb.push(ch as u8 as char);
                cur_node = pairs.len() - 1;
            } else {
                cur_node = next as usize;
                if cur_node > pairs.len() {
                    return Err(CoalescedError::MalformedDecompressionNodes);
                }
            }

            pos += 1;
        }

        Ok(sb)
    }

    fn build_tree(freq: FrequencyMap) -> HuffmanTree {
        // Create the initial leafs for each character value
        let mut heap = BinaryHeap::new();
        for (char, freq) in freq.0 {
            heap.push(HuffmanTree::Leaf(char, freq));
        }

        // Flatten the leafs into a tree
        while heap.len() > 1 {
            let left = heap.pop().unwrap();
            let right = heap.pop().unwrap();

            heap.push(HuffmanTree::Node(Box::new(left), Box::new(right)));
        }

        heap.pop().unwrap()
    }

    /// Creates the combination of bits that represents each character by
    /// traversing the huffman tree storing the path that it took to get
    /// there.
    fn generate_huffman_codes(
        node: &HuffmanTree,
        prefix: BitVec,
        codes: &mut HashMap<char, BitVec>,
    ) {
        match node {
            HuffmanTree::Node(left, right) => {
                let mut left_prefix = prefix.clone();
                left_prefix.push(false);
                Self::generate_huffman_codes(left, left_prefix, codes);

                let mut right_prefix = prefix;
                right_prefix.push(true);
                Self::generate_huffman_codes(right, right_prefix, codes);
            }
            HuffmanTree::Leaf(char, _) => {
                codes.insert(*char, prefix);
            }
        }
    }

    /// Flattens the tree of huffman nodes into an array of pairs where:
    ///
    /// - Negative values represent the actual character literal
    /// - Positive values represent the next index to visit
    ///
    /// When decoding the decoder uses the encoded bit to decide which
    /// half of the pair it should use, encoding characters when it hits
    /// the negative values and continuing to the target pair when hitting
    /// a positive value
    fn collect_pairs(root: &HuffmanTree) -> Vec<(i32, i32)> {
        // Actual pairs themselves (Not the correct order)
        let mut pairs_unordered: Vec<(i32, i32)> = Vec::new();

        // References to the actual order of inserted pairs (Index into unordered list)
        let mut pair_refs: Vec<usize> = Vec::new();

        // References to pairs based on their huffman tree node/leaf (Index into unordered list)
        let mut tree_ref: HashMap<*const HuffmanTree, usize> = HashMap::new();

        // Queue of nodes to process
        let mut queue: VecDeque<&HuffmanTree> = VecDeque::new();

        // Pushes a new pair returning its index
        let push_pair = |pairs: &mut Vec<(i32, i32)>, pair: (i32, i32)| {
            let pair_index = pairs.len();
            pairs.push(pair);
            pair_index
        };

        // Push root un-ordered pair
        let root_pair = push_pair(&mut pairs_unordered, (0, 0));
        tree_ref.insert(root, root_pair);

        queue.push_back(root);

        while let Some(node) = queue.pop_front() {
            let node_index = *tree_ref
                .get(&(node as *const _))
                .expect("Missing mapping for current node");

            let current_index = pair_refs.len() as i32;

            let HuffmanTree::Node(left_node, right_node) = node else {
                // Not a possible state unless the implementation is broken
                panic!("Invalid operation: leaf node in queue")
            };

            let left_value = &mut pairs_unordered[node_index].0;

            if let HuffmanTree::Leaf(symbol, _) = left_node.as_ref() {
                *left_value = -1 - *symbol as i32;
            } else {
                // Update previous pair
                *left_value = current_index;

                // Add empty left pair
                let pair_index = push_pair(&mut pairs_unordered, (0, 0));

                tree_ref.insert(left_node.as_ref(), pair_index);
                pair_refs.push(pair_index);

                // Queue the left node
                queue.push_back(left_node.as_ref());
            }

            let right_value = &mut pairs_unordered[node_index].1;

            if let HuffmanTree::Leaf(symbol, _) = right_node.as_ref() {
                *right_value = -1 - *symbol as i32;
            } else {
                // Update previous pair
                *right_value = current_index;

                // Add empty left pair
                let pair_index = push_pair(&mut pairs_unordered, (0, 0));

                tree_ref.insert(right_node.as_ref(), pair_index);
                pair_refs.push(pair_index);

                // Queue the left node
                queue.push_back(right_node.as_ref());
            }
        }

        // Push the root pair
        pair_refs.push(root_pair);

        // Collect the actual pairs using the refs to unordered mapping
        pair_refs
            .into_iter()
            .map(|index| pairs_unordered[index])
            .collect()
    }
}
