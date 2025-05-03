use std::fmt;
use std::mem;

// Type of the symbols used in the Huffman tree.
type SymbolType = u16;

// Type of the weights used in the Huffman tree.
type WeightType = u32;

const DEBUG: bool = false;

// Keeps track of the weight and symbol.
#[derive(Debug, Copy, Clone)]
struct HeapNode {
    // This is (raw weight of the symbol * 2) + (1 if it's an internal node).
    // This is to allow leaf nodes to be merged first before internal nodes of equal raw weight.
    weight: WeightType,

    // Leaf symbols followed by internal node ids.
    symbol: SymbolType,
}

impl HeapNode {
    fn new(symbol: SymbolType, weight: WeightType) -> Self {
        Self { weight, symbol }
    }
}

// Keeps track of the parent and level of the node.
#[derive(Clone)]
struct ParentNode {
    parent: SymbolType,
    level: u8,
}

// Heapify the node at pos up to the root.
fn heapify_up(heap: &mut [HeapNode], mut pos: usize) {
    let orig_node = heap[pos];
    let weight = orig_node.weight;

    while pos > 0 {
        let parent = (pos - 1) / 2;
        if heap[parent].weight <= weight {
            break;
        }
        heap[pos] = heap[parent];
        pos = parent;
    }
    heap[pos] = orig_node;
}

// Heapify down.
fn heapify_down(heap: &mut [HeapNode], size: usize, insert_node: HeapNode) {
    let mut pos = 0;
    loop {
        let left = pos * 2 + 1;
        let right = left + 1;

        let smaller = if right < size {
            if heap[left].weight <= heap[right].weight {
                left
            } else {
                right
            }
        } else if left < size {
            left
        } else {
            break;
        };

        if insert_node.weight <= heap[smaller].weight {
            break;
        }
        heap[pos] = heap[smaller];
        pos = smaller;
    }
    heap[pos] = insert_node;
}

// Check that the heap is valid.
fn check_heap(heap: &[HeapNode], size: usize) {
    if DEBUG {
        for i in 0..size {
            let left = i * 2 + 1;
            let right = left + 1;
            if left < size {
                assert!(heap[i].weight <= heap[left].weight);
            }
            if right < size {
                assert!(heap[i].weight <= heap[right].weight);
            }
        }
    }
}

/// Code lengths for each symbol.
#[derive(Clone)]
pub struct CodeLengths {
    // lengths[i] = symbols of length i.
    lengths: Vec<Vec<SymbolType>>,
}

impl CodeLengths {
    /// Sets the maximum code length to `max_length`.
    pub fn apply_max_length_limit(&mut self, max_length: usize) {
        println!("apply_max_length_limit: {} max: {}", self, max_length);
        if max_length >= self.lengths.len() - 1 {
            return;
        }

        // Count the extra weight due to moving longest symbols to `max_length`.
        let mut delta: usize = 0;
        for level in max_length + 1..self.lengths.len() {
            let num_symbols = self.lengths[level].len();
            delta += self.weight_delta(num_symbols, max_length, level);

            // Move the symbols to max_length.
            let symbols = mem::take(&mut self.lengths[level]);
            self.lengths[max_length].extend(symbols);
        }

        // Rebalance the prefix tree, moving some symbols down (i.e. increasing code length).
        let mut delta_to_adjust = delta;
        self.adjust(max_length - 1, max_length, 0, &mut delta_to_adjust);
        assert!(delta_to_adjust == 0);

        self.lengths.truncate(max_length + 1);
    }

    // Weight changes when moving `num` symbols at `higher` level of the tree to `lower`
    // level (i.e. shorter code to longer code).
    fn weight_delta(&self, num: usize, higher: usize, lower: usize) -> usize {
        let longest = self.lengths.len() - 1;
        (num << (longest - higher)) - (num << (longest - lower)) 
    }

    // Adjust the symbols at specified `level`.
    fn adjust(
        &mut self,
        level: usize,
        max_length: usize,
        total_adjust: usize,
        delta: &mut usize,
    ) {
        // Recursively go up the tree (decreasing code length) until the highest level where
        // adjustment is necessary.
        let num_symbols = self.lengths[level].len();

        // Find the maximum weight adjustment possible from bottom up to this level.
        let max_adjust = self.weight_delta(num_symbols, level, max_length);
        let new_total_adjust = total_adjust + max_adjust;
        if new_total_adjust < *delta as usize {
            assert!(level > 0, "Not possible to apply specified length limit");

            // Need to recurse upwards.
            self.adjust(level - 1, max_length, new_total_adjust, delta);
        }

        // Make adjustments at this level by moving some symbols to (level + 1).
        if *delta > 0 {
            let adjustment = self.weight_delta(1, level, level + 1);
            while *delta > total_adjust && !self.lengths[level].is_empty() {
                let symbol = self.lengths[level].pop().unwrap();
                self.lengths[level + 1].push(symbol);
                *delta -= adjustment;
            }
        }
    }
}

impl fmt::Display for CodeLengths {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.lengths)
    }
}

/// Static Huffman encoder/decoder.
pub struct StaticHuffman {
    num_symbols: SymbolType,
}

impl StaticHuffman {
    pub fn new(num_symbols: SymbolType) -> Self {
        Self { num_symbols }
    }

    // Build the huffman code table from the weights of the symbols.
    // Returns the code lengths for each symbol.
    pub fn build_from_weights(&self, weights: &[WeightType]) -> CodeLengths {
        assert!(weights.len() == self.num_symbols as usize);

        // Use a heap to extract smallest weight nodes.
        let mut table: Vec<HeapNode> = Vec::with_capacity(self.num_symbols as usize);

        // Map of non-zero-weight symbols.
        let mut symbols: Vec<SymbolType> = Vec::with_capacity(self.num_symbols as usize);

        // Add non-zero weights to the heap.
        for i in 0..self.num_symbols {
            if weights[i as usize] > 0 {
                table.push(HeapNode::new(
                    symbols.len() as SymbolType,
                    weights[i as usize] << 1,
                ));
                symbols.push(i);
            }
        }

        // Number of non-zero-weight symbols.
        let symbol_size = table.len();
        assert!(symbol_size > 0);

        // Heapify the table.
        for i in 1..symbol_size {
            heapify_up(&mut table, i);
        }
        check_heap(&table, symbol_size);

        // parent[i] = parent of symbol i.
        let mut parents: Vec<ParentNode> = vec![
            ParentNode {
                parent: 0,
                level: 0
            };
            symbol_size * 2
        ];

        // Internal nodes start at this index.
        let mut parent_index = symbol_size as SymbolType;

        // Repeatedly create parent nodes from the two lowest weight nodes.
        let mut size = symbol_size;
        while size >= 2 {
            // Remove the two lowest weight nodes (left and right) from the heap.
            size -= 1;
            let left = table[0];
            let last_node = table[size];
            heapify_down(&mut table, size, last_node);
            let right = table[0];

            // Link each child to its parent.
            const MASK: WeightType = !1;
            let parent_weight = (left.weight & MASK) + (right.weight & MASK) + 1;
            let parent_node = HeapNode::new(parent_index, parent_weight);
            parents[left.symbol as usize].parent = parent_index;
            parents[right.symbol as usize].parent = parent_index;
            parent_index += 1;

            // Insert the parent node into the heap.
            heapify_down(&mut table, size, parent_node);
            check_heap(&table, size);
        }

        // Calculate the level of each internal node by traversing down from the root.
        for i in (symbol_size..symbol_size * 2 - 2).rev() {
            let parent = parents[i].parent;
            parents[i].level = parents[parent as usize].level + 1;
        }

        // Output the code lengths.
        let mut lengths: Vec<Vec<SymbolType>> = Vec::new();
        for i in 0..symbol_size as SymbolType {
            if DEBUG {
                println!(
                    "i:{}, parent:{}, level:{}",
                    i, parents[i as usize].parent, parents[i as usize].level
                );
            }
            let parent = parents[i as usize].parent;
            let level = (parents[parent as usize].level + 1) as usize;
            while level >= lengths.len() {
                lengths.push(Vec::new());
            }
            lengths[level].push(symbols[i as usize]);
        }
        CodeLengths { lengths }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs, Rng, SeedableRng};
    use std::collections::HashSet;

    // Check that the code lengths are properly assigned such that the huffman tree is full.
    fn validate_code_lengths(code_lengths: &CodeLengths) {
        assert!(code_lengths.lengths[0].len() == 0);
        let lengths = &code_lengths.lengths;
        let mut sum: u64 = 0;
        let mut weight = 1u64 << 62;
        let mut seen: HashSet<SymbolType> = HashSet::new();
        let mut num_symbols = 0;
        for i in 1..lengths.len() {
            sum += lengths[i].len() as u64 * weight;
            weight = weight >> 1;
            num_symbols += lengths[i].len();

            // Check that the symbols are unique.
            for symbol in lengths[i].iter() {
                assert!(!seen.contains(symbol));
                seen.insert(*symbol);
            }
        }
        if num_symbols == 1 {
            // Special case for a single symbol.
            assert!(sum == 1 << 62);
        } else {
            assert!(sum == 1 << 63);
        }
    }

    #[test]
    fn test_simple() {
        let huffman = StaticHuffman::new(12);
        let weights = vec![1, 3, 0, 10, 9, 8, 6, 0, 7, 5, 4, 2];
        let code_lengths = huffman.build_from_weights(&weights);
        println!("{}", &code_lengths);
        validate_code_lengths(&code_lengths);
    }

    #[test]
    fn test_single_symbol() {
        let huffman = StaticHuffman::new(2);
        let weights = vec![0, 1];
        let code_lengths = huffman.build_from_weights(&weights);
        println!("{}", &code_lengths);
        validate_code_lengths(&code_lengths);
    }

    #[test]
    fn test_merge_leaf_nodes_first() {
        let huffman = StaticHuffman::new(6);
        let weights = vec![2, 2, 2, 2, 4, 4];
        let code_lengths = huffman.build_from_weights(&weights);
        println!("{}", &code_lengths);
        validate_code_lengths(&code_lengths);
    }

    #[test]
    fn test_random() {
        let huffman = StaticHuffman::new(256);
        for s in 0..100 {
            let mut rng = rngs::SmallRng::seed_from_u64(s);
            let mut weights = Vec::new();
            for _ in 0..256 {
                let weight = rng.gen::<WeightType>() / 1000;
                weights.push(weight);
            }
            let code_lengths = huffman.build_from_weights(&weights);
            validate_code_lengths(&code_lengths);
        }
    }

    #[test]
    fn test_apply_max_length_limit() {
        fn test(code_lengths: &mut CodeLengths, max_lengths: &[usize]) {
            for max_length in max_lengths {
                let mut copy = code_lengths.clone();
                validate_code_lengths(&copy);
                copy.apply_max_length_limit(*max_length);
                println!("{}", &copy);
                assert!(copy.lengths.len() <= *max_length + 1);
                validate_code_lengths(&copy);
            }
        }

        test(
            &mut CodeLengths {
                lengths: vec![vec![], vec![0], vec![1], vec![2, 3]],
            },
            &[2, 3, 4],
        );

        test(
            &mut CodeLengths {
                lengths: vec![vec![], vec![0], vec![1], vec![2], vec![3], vec![4, 5]],
            },
            &[3, 4, 5],
        );

        test(
            &mut CodeLengths {
                lengths: vec![vec![], vec![0], vec![1], vec![], vec![2, 3, 4, 5]],
            },
            &[3, 4],
        );

        test(
            &mut CodeLengths {
                lengths: vec![
                    vec![],
                    vec![0],
                    vec![],
                    vec![1],
                    vec![],
                    vec![],
                    vec![
                        2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                        23, 24, 25,
                    ],
                ],
            },
            &[5, 6],
        );

        test(
            &mut CodeLengths {
                lengths: vec![
                    vec![],
                    vec![0],
                    vec![1],
                    vec![2],
                    vec![3],
                    vec![4],
                    vec![5, 6],
                ],
            },
            &[3],
        );

        test(
            &mut CodeLengths {
                lengths: vec![
                    vec![],
                    vec![0],
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    (1..33).collect::<Vec<SymbolType>>(),
                ],
            },
            &[6],
        );
    }
}
