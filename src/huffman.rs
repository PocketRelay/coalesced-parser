use bitvec::{access::BitSafeU8, order::Lsb0, vec::BitVec};
use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, VecDeque},
    rc::Rc,
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

pub struct Huffman {
    mapping: HashMap<char, BitVec>,
    pairs: Vec<(i32, i32)>,
}

impl Huffman {
    pub fn new(text: &str) -> Self {
        let huffman_tree = Self::build_tree(text);
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

    fn build_tree(text: &str) -> HuffmanTree {
        // Get the frequency for each character
        let mut frequency_map = HashMap::new();
        for c in text.chars() {
            *frequency_map.entry(c).or_insert(0) += 1;
        }

        // Create the initial leafs for each character value
        let mut heap = BinaryHeap::new();
        for (char, freq) in frequency_map {
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

    /// Flattens the tree of huffman nodes into pairs where negative values are the symbols and
    /// positive values are the next node index
    fn collect_pairs(root: &HuffmanTree) -> Vec<(i32, i32)> {
        let mut pairs: Vec<Rc<RefCell<(i32, i32)>>> = Vec::new();
        let mut mapping: HashMap<*const HuffmanTree, Rc<RefCell<(i32, i32)>>> = HashMap::new();
        let mut queue: VecDeque<&HuffmanTree> = VecDeque::new();

        let root_pair = Rc::new(RefCell::new((0, 0)));

        mapping.insert(root, root_pair.clone());
        queue.push_back(root);

        while let Some(node) = queue.pop_front() {
            let item = mapping.get(&(node as *const _)).unwrap().clone();

            if let HuffmanTree::Node(left_node, right_node) = node {
                if let HuffmanTree::Leaf(symbol, _) = left_node.as_ref() {
                    item.borrow_mut().0 = -1 - *symbol as i32;
                } else {
                    let left = Rc::new(RefCell::new((0, 0)));

                    // Add empty left pair
                    mapping.insert(left_node.as_ref(), left.clone());
                    pairs.push(left.clone());

                    // Queue the left node
                    queue.push_back(left_node.as_ref());

                    {
                        item.borrow_mut().0 = (pairs.len() - 1) as i32;
                    }
                }

                if let HuffmanTree::Leaf(symbol, _) = right_node.as_ref() {
                    item.borrow_mut().1 = -1 - *symbol as i32;
                } else {
                    let right = Rc::new(RefCell::new((0, 0)));

                    // Add empty right pair
                    mapping.insert(right_node.as_ref(), right.clone());
                    pairs.push(right.clone());

                    queue.push_back(right_node.as_ref());

                    {
                        item.borrow_mut().1 = (pairs.len() - 1) as i32;
                    }
                }
            } else {
                // Not a possible state unless the implementation is broken
                panic!("Invalid operation: leaf node in queue");
            }
        }
        pairs.push(root_pair);

        let pairs = pairs.into_iter().map(|value| *value.borrow()).collect();
        pairs
    }
}
