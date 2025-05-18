use crate::huffman::prefix_code::PrefixCode;

// Type of the symbols used in the Huffman tree.
type SymbolType = u16;

// Type of the weights used in the Huffman tree.
type WeightType = u32;

// If true, print more debug information.
const DEBUG: bool = false;

// Keeps track of the weight and symbol of a node in a heap.
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
    // The parent node index.
    parent: SymbolType,

    // The tree level.
    level: u8,
}

// Heapify the node at `pos`` up to the root.
fn heapify_up(heap: &mut [HeapNode], mut pos: usize) {
    let orig_node = heap[pos];
    let weight = orig_node.weight;

    while pos > 0 {
        let parent = (pos - 1) >> 1;
        if heap[parent].weight <= weight {
            break;
        }
        heap[pos] = heap[parent];
        pos = parent;
    }
    heap[pos] = orig_node;
}

// Replace the head of the heap with `insert_node`.
fn heapify_down(heap: &mut [HeapNode], size: usize, insert_node: HeapNode) {
    let mut pos = 0;
    loop {
        let left = (pos << 1) + 1;
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

/// A static Huffman encoder/decoder.
pub struct StaticHuffman {
    num_symbols: SymbolType,
}

impl StaticHuffman {
    pub fn new(num_symbols: SymbolType) -> Self {
        Self { num_symbols }
    }

    /// Builds the huffman code table from the weights of the symbols.
    /// Returns the code lengths for each symbol.
    pub fn build_from_weights(&self, weights: &[WeightType]) -> PrefixCode {
        assert!(weights.len() == self.num_symbols as usize);

        // Use a heap to extract smallest weight nodes while building the tree.
        let mut table: Vec<HeapNode> = Vec::with_capacity(self.num_symbols as usize);

        // Contains the non-zero-weight symbols.
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

            // Parent weight = sum of children weight, with last bit (meaning non-leaf) set to 1.
            const MASK: WeightType = !1;
            let parent_weight = (left.weight & MASK) + (right.weight | 1);

            // Link each child to its parent.
            let parent_node = HeapNode::new(parent_index, parent_weight);
            parents[left.symbol as usize].parent = parent_index;
            parents[right.symbol as usize].parent = parent_index;
            parent_index += 1;

            // Insert the parent node into the heap.
            heapify_down(&mut table, size, parent_node);
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
        PrefixCode::new(self.num_symbols, lengths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::huffman::prefix_code::tests::validate_prefix_code;
    use rand::{rngs, Rng, SeedableRng};

    #[test]
    fn test_simple() {
        let huffman = StaticHuffman::new(12);
        let weights = vec![1, 3, 0, 10, 9, 8, 6, 0, 7, 5, 4, 2];
        let prefix_code = huffman.build_from_weights(&weights);
        println!("{}", &prefix_code);
        validate_prefix_code(&prefix_code);
    }

    #[test]
    fn test_single_symbol() {
        let huffman = StaticHuffman::new(2);
        let weights = vec![0, 1];
        let prefix_code = huffman.build_from_weights(&weights);
        println!("{}", &prefix_code);
        validate_prefix_code(&prefix_code);
    }

    #[test]
    fn test_merge_leaf_nodes_first() {
        let huffman = StaticHuffman::new(6);
        let weights = vec![2, 2, 2, 2, 4, 4];
        let prefix_code = huffman.build_from_weights(&weights);
        println!("{}", &prefix_code);
        validate_prefix_code(&prefix_code);
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
            let prefix_code = huffman.build_from_weights(&weights);
            validate_prefix_code(&prefix_code);
        }
    }
}
