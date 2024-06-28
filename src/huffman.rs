use bitvec::{access::BitSafeU8, order::Lsb0, vec::BitVec};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, VecDeque},
    hash::Hash,
};

use crate::{error::CoalescedError, WChar, WString};

/// Represents a node/leaf within a huffman tree
#[derive(Debug)]
enum HuffmanTree<C> {
    /// Node with a left and right path
    Node(Box<HuffmanTree<C>>, Box<HuffmanTree<C>>),
    /// Leaf with a value and frequency
    Leaf(C, u32),
}

impl<C> HuffmanTree<C> {
    /// Gets the frequency of this huffman tree node/leaf, for leafs this is
    /// the value of the leaf for nodes this is the sum of both halves
    fn frequency(&self) -> u32 {
        match *self {
            HuffmanTree::Node(ref left, ref right) => left.frequency() + right.frequency(),
            HuffmanTree::Leaf(_, freq) => freq,
        }
    }
}

impl<C> PartialEq for HuffmanTree<C> {
    fn eq(&self, other: &Self) -> bool {
        self.frequency().eq(&other.frequency())
    }
}

impl<C> Eq for HuffmanTree<C> {}

impl<C> Ord for HuffmanTree<C> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.frequency().cmp(&other.frequency()).reverse()
    }
}

impl<C> PartialOrd for HuffmanTree<C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Map containing character frequencies to build a huffman tree from
#[derive(Default)]
pub(crate) struct FrequencyMap<C: HuffmanChar>(HashMap<C, u32>);

impl<C: HuffmanChar> FrequencyMap<C> {
    /// Updates the frequency map from the provided iterator
    pub fn push_iter<I: IntoIterator<Item = C>>(&mut self, iter: I) {
        iter.into_iter().for_each(|value| self.push(value))
    }

    /// Updates the frequency map for the provided character
    #[inline]
    pub fn push(&mut self, value: C) {
        *self.0.entry(value).or_insert(0) += 1;
    }
}

/// Trait implemented by types that can be decoded as strings
/// by the huffman encoding
pub trait HuffmanString: 'static {
    /// Character type this string uses
    type Char: HuffmanChar;

    /// Creates a new string instance
    fn new() -> Self;

    /// Appends a char to the string
    fn append_char(&mut self, value: Self::Char);

    /// Gets the length of the string
    fn len(&self) -> usize;
}

impl HuffmanString for String {
    type Char = char;

    #[inline]
    fn new() -> Self {
        String::new()
    }

    #[inline]
    fn append_char(&mut self, value: Self::Char) {
        self.push(value)
    }

    #[inline]
    fn len(&self) -> usize {
        self.len()
    }
}

impl HuffmanString for WString {
    type Char = WChar;

    #[inline]
    fn new() -> Self {
        WString::new()
    }

    #[inline]
    fn append_char(&mut self, value: Self::Char) {
        self.push(value)
    }

    #[inline]
    fn len(&self) -> usize {
        self.len()
    }
}

/// Trait implemented by types that can be used as an individual
/// character within the huffman encoding
pub trait HuffmanChar: Hash + PartialEq + Eq + Copy + 'static {
    /// Character representing null for this type
    const NULL: Self;

    /// Converts the value into a huffman symbol
    fn as_symbol(self) -> i32;

    /// Creates a char from a huffman symbol
    fn from_symbol(value: i32) -> Self;
}

impl HuffmanChar for char {
    const NULL: Self = '\0';

    #[inline]
    fn as_symbol(self) -> i32 {
        self as i32
    }

    #[inline]
    fn from_symbol(value: i32) -> Self {
        value as u8 as char
    }
}

impl HuffmanChar for WChar {
    const NULL: Self = 0;

    #[inline]
    fn as_symbol(self) -> i32 {
        self as i32
    }

    #[inline]
    fn from_symbol(value: i32) -> Self {
        value as WChar
    }
}

/// Huffman encoding state
pub(crate) struct Huffman<C: HuffmanChar> {
    /// Mapping from chars to their huffman encoded bits
    mapping: HashMap<C, BitVec>,
    /// Flattened pairs from the huffman tree
    pairs: Vec<(i32, i32)>,
}

impl<C: HuffmanChar> Huffman<C> {
    /// Creates a new huffman encoder from the provided frequency map
    pub fn new(freq: FrequencyMap<C>) -> Self {
        let huffman_tree = Self::build_tree(freq);
        let mapping = Self::generate_huffman_codes(&huffman_tree);
        let pairs = Self::collect_pairs(&huffman_tree);

        Self { mapping, pairs }
    }

    /// Get a reference to the pairs for encoding
    pub fn get_pairs(&self) -> &[(i32, i32)] {
        &self.pairs
    }

    /// Writes the huffman encoding bits representing the input text to the
    /// provided output buffer
    pub fn encode<I: IntoIterator<Item = C>>(&self, iter: I, output: &mut BitVec<BitSafeU8, Lsb0>) {
        iter.into_iter()
            .filter_map(|code| self.mapping.get(&code))
            .for_each(|value| output.extend(value))
    }

    /// Helper to encode null bytes
    pub fn encode_null(&self, output: &mut BitVec<BitSafeU8, Lsb0>) {
        let code = self
            .mapping
            .get(&C::NULL)
            .expect("Missing null byte encoding");
        output.extend(code);
    }

    /// Decodes huffman encoded text
    pub fn decode<S: HuffmanString<Char = C>>(
        compressed_data: &[u8],
        pairs: &[(i32, i32)],
        position: usize,
        max_length: usize,
    ) -> Result<S, CoalescedError> {
        let mut sb = S::new();
        let mut cur_node = pairs.len() - 1;
        let end = compressed_data.len() * 8;

        let mut pos = position;

        while pos < end && sb.len() < max_length {
            let sample = compressed_data[pos / 8] & (1 << (pos % 8));
            let next = pairs[cur_node];
            let next = if sample != 0 { next.1 } else { next.0 };

            if next < 0 {
                let ch = -1 - next;
                if ch == 0 {
                    break;
                }
                sb.append_char(S::Char::from_symbol(ch));
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

    /// Builds a huffman tree root node from the provided
    /// frequency map
    fn build_tree(freq: FrequencyMap<C>) -> HuffmanTree<C> {
        // Create the initial leafs for each character value
        let mut heap = BinaryHeap::new();
        for (char, freq) in freq.0 {
            heap.push(HuffmanTree::Leaf(char, freq));
        }

        // Handle empty frequencies
        if heap.is_empty() {
            return HuffmanTree::Leaf(C::NULL, 0);
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
    fn generate_huffman_codes(node: &HuffmanTree<C>) -> HashMap<C, BitVec> {
        let mut codes = HashMap::new();
        let mut stack = VecDeque::new();
        stack.push_back((node, BitVec::new()));

        while let Some((current_node, prefix)) = stack.pop_back() {
            match current_node {
                HuffmanTree::Node(left, right) => {
                    let mut left_prefix = prefix.clone();
                    left_prefix.push(false);
                    stack.push_back((left, left_prefix));

                    let mut right_prefix = prefix;
                    right_prefix.push(true);
                    stack.push_back((right, right_prefix));
                }
                HuffmanTree::Leaf(char, _) => {
                    codes.insert(*char, prefix);
                }
            }
        }

        codes
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
    fn collect_pairs(root: &HuffmanTree<C>) -> Vec<(i32, i32)> {
        // Actual pairs themselves (Not the correct order)
        let mut pairs_unordered: Vec<(i32, i32)> = Vec::new();

        // References to the actual order of inserted pairs (Index into unordered list)
        let mut pair_refs: Vec<usize> = Vec::new();

        // References to pairs based on their huffman tree node/leaf (Index into unordered list)
        let mut tree_ref: HashMap<*const HuffmanTree<C>, usize> = HashMap::new();

        // Queue of nodes to process
        let mut queue: VecDeque<&HuffmanTree<C>> = VecDeque::new();

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

            let HuffmanTree::Node(left_node, right_node) = node else {
                // Not a possible state unless the implementation is broken
                panic!("Invalid operation: leaf node in queue")
            };

            let left_value = &mut pairs_unordered[node_index].0;

            if let HuffmanTree::Leaf(symbol, _) = left_node.as_ref() {
                *left_value = -1 - (*symbol).as_symbol();
            } else {
                // Update previous pair
                *left_value = pair_refs.len() as i32;

                // Add empty left pair
                let pair_index = push_pair(&mut pairs_unordered, (0, 0));

                tree_ref.insert(left_node.as_ref(), pair_index);
                pair_refs.push(pair_index);

                // Queue the left node
                queue.push_back(left_node.as_ref());
            }

            let right_value = &mut pairs_unordered[node_index].1;

            if let HuffmanTree::Leaf(symbol, _) = right_node.as_ref() {
                *right_value = -1 - (*symbol).as_symbol();
            } else {
                // Update previous pair
                *right_value = pair_refs.len() as i32;

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
