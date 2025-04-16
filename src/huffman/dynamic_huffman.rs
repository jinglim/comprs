use crate::base::DebugLog;
use crate::bits::{BitReader, BitWriter};

use std::io;

// If true, print debug information.
const DEBUG: bool = true;

// If true, print tree details for debugging.
const DEBUG_TREE: bool = false;

// If true, print nodes in linear order.
const DEBUG_SORTED_NODES: bool = false;

// Debug log.
const LOG: DebugLog = DebugLog::new("DynamicHuffman");

// Type of the weight values.
type WeightType = u32;

// Weight value that triggers a reset of all the weights in the tree.
// (Minus 2 because we increment the weight by 2 each time.)
const RESET_WEIGHT: WeightType = WeightType::MAX - 2;

// Symbol for NYT (Not Yet Transmitted) symbol.
const NYT_SYMBOL: u16 = 0;

// A dynamic huffman tree, based on Vitter's algorithm.
pub struct DynamicHuffman {
    // Nodes[0..num_symbols] are the symbol nodes, each corresponding to a symbol.
    // Nodes[num_symbols + 1 ..] are the leaf and internal nodes sorted in descending
    // weight order.
    nodes: Vec<Node>,

    // Number of symbols in the symbol set.
    num_symbols: u16,

    // Log2 of num_symbols.
    symbol_bits: u32,
}

#[derive(Clone, Debug)]
struct Node {
    // The lsb of the `weight` is used to indicate if the node is a leaf node.
    // Internal nodes have lsb set to 1, while leaf nodes have lsb set to 0.
    // This allows internal nodes to be ordered after the leaf nodes of the same
    // raw weight, when sorted by weight.
    // (weight >> 1) is the raw weight of a node in a typical huffman tree.
    weight: WeightType,

    // Index of the parent node. 0 means no parent.
    parent: u16,

    // Node id of the left child. Right child id = child + 1.
    child: u16,
}

impl DynamicHuffman {
    // Create a new instance with a given number of symbols.
    pub fn new(num_symbols: u16) -> Self {
        assert!(num_symbols > 0);

        // Compute number of bits needed to represent the symbols.
        let mut symbol_bits = 0u32;
        while (1 << symbol_bits) < num_symbols {
            symbol_bits += 1;
        }

        // Max number of nodes: N + 1 symbol nodes, N + 1 leaf nodes, N internal nodes.
        let mut nodes: Vec<Node> = Vec::with_capacity((num_symbols as usize) * 3 + 2);
        Self::initialize_nodes(&mut nodes, num_symbols);

        Self {
            nodes,
            num_symbols,
            symbol_bits,
        }
    }

    // The root node in the tree. All other tree nodes come after this.
    fn root_node_id(&self) -> u16 {
        self.num_symbols + 1
    }

    // Helper function to initialize the nodes.
    fn initialize_nodes(nodes: &mut Vec<Node>, num_symbols: u16) {
        // Add the symbol nodes and the root node.
        nodes.resize(
            (num_symbols + 1) as usize,
            Node {
                // Set max weight so they are greater than any other regular tree node.
                weight: WeightType::MAX,
                parent: 0,
                child: 0,
            },
        );

        // Add the NYT node.
        nodes.push(Node {
            weight: 0,
            parent: 0,
            child: NYT_SYMBOL,
        });
    }

    // Reset the entire tree, if necessary, to avoid weight overflow.
    fn reset_if_necessary(&mut self) {
        if self.nodes[self.root_node_id() as usize].weight > RESET_WEIGHT {
            println!("Resetting tree");
            self.nodes.clear();
            Self::initialize_nodes(&mut self.nodes, self.num_symbols);
        }
    }

    // Encode a symbol.
    pub fn encode(&mut self, symbol: u16, bit_writer: &mut BitWriter) {
        assert!(symbol < self.num_symbols);
        if DEBUG {
            LOG.print(&format!("Encode: {}", symbol));
        }

        let node_id = self.nodes[(symbol + 1) as usize].parent; // +1 to skip NYT symbol.
        if node_id != 0 {
            // Symbol has been transmitted before.
            self.output_code(node_id, bit_writer);
            self.slide_and_increment_loop(node_id);
        } else {
            // Symbol has not been transmitted yet.
            let nyt_id = (self.nodes.len() - 1) as u16;
            if nyt_id == self.root_node_id() {
                // For the first symbol, just output the raw symbol.
                self.output_raw_symbol(symbol, bit_writer);
            } else {
                // Output the NYT symbol followed by the raw symbol.
                self.output_code(nyt_id, bit_writer);
                self.output_raw_symbol(symbol, bit_writer);
            }
            self.add_new_symbol(symbol);
        }

        // Don't let the total weight of the tree overflow.
        self.reset_if_necessary();
    }

    // Decode a symbol.
    pub fn decode(&mut self, bit_reader: &mut BitReader) -> u16 {
        if bit_reader.bits_avail() < 16 {
            bit_reader.fill_data();
        }
        let mut data = bit_reader.peek();
        let mut bits_avail = bit_reader.bits_avail();

        if DEBUG {
            LOG.print(&format!("Decode {:#x} bits avail: {}", data, bits_avail));
        }

        // Traverse from the root node down to the leaf node.
        let mut bits_consumed = 0;
        let mut node_id = self.root_node_id();
        let mut child_id;
        loop {
            child_id = self.nodes[node_id as usize].child;
            if child_id <= self.num_symbols {
                // Reached the leaf node.
                break;
            }

            // If the msb is 1, then go to the right child.
            let msb = (data >> 63) as u16;
            node_id = child_id + msb;
            data <<= 1;

            bits_consumed += 1;
            if bits_consumed == bits_avail {
                if DEBUG {
                    LOG.print("Consumed all bits");
                }
                bit_reader.consume(bits_avail);
                bit_reader.fill_data();
                data = bit_reader.peek();
                bits_consumed = 0;
                bits_avail = bit_reader.bits_avail();
            }
        }

        bit_reader.consume(bits_consumed);

        // If the decoded symbol is the NYT symbol, then read the raw symbol.
        let decoded_symbol = if child_id == NYT_SYMBOL {
            let new_symbol = bit_reader.read_bits(self.symbol_bits) as u16;
            self.add_new_symbol(new_symbol);
            new_symbol
        } else {
            // Update the tree.
            self.slide_and_increment_loop(node_id);
            child_id - 1
        };

        // Don't let the total weight of the tree overflow.
        self.reset_if_necessary();

        decoded_symbol
    }

    // Symbol does not exist, add it to the tree.
    fn add_new_symbol(&mut self, symbol: u16) {
        // Add two child nodes with their parent = the original NYT node.
        // So, starting with: [..., NYT]
        // After this: [..., parent, new symbol leaf, new NYT]
        let nyt_id = (self.nodes.len() - 1) as u16;
        let nyt_node = &mut self.nodes[nyt_id as usize];
        nyt_node.weight = 1;
        nyt_node.child = nyt_id + 1;

        // Add the leaf node for the new symbol.
        self.nodes.push(Node {
            weight: 2,
            parent: nyt_id,
            child: symbol + 1,
        });
        self.nodes[(symbol + 1) as usize].parent = nyt_id + 1;

        // Add the new NYT.
        self.nodes.push(Node {
            weight: 0,
            parent: nyt_id,
            child: NYT_SYMBOL,
        });

        // Update the tree.
        self.slide_and_increment_loop(nyt_id);
    }

    // Increment the weight of the node and update recursively up the tree.
    fn slide_and_increment_loop(&mut self, mut node_id: u16) {
        loop {
            node_id = self.slide_and_increment(node_id);
            if node_id == 0 {
                break;
            }
        }
    }

    // Increment the weight of the node and return the parent that needs to be updated.
    fn slide_and_increment(&mut self, mut node_id: u16) -> u16 {
        let node = &mut self.nodes[node_id as usize];
        let weight = node.weight;
        node.weight += 2;
        let mut parent_id = node.parent;

        // Check if this node needs to be moved to the left.
        let mut prev_id = node_id - 1;
        let mut prev_weight = self.nodes[prev_id as usize].weight;
        if prev_weight >= weight + 2 || prev_id == parent_id {
            return parent_id;
        }

        // Find the leader of the group with same weight.
        while prev_weight == weight {
            prev_id -= 1;
            prev_weight = self.nodes[prev_id as usize].weight;
        }
        prev_id += 1;

        // Swap the node with the leader of the group with same weight.
        if prev_id < node_id {
            assert!(parent_id < prev_id);
            self.swap_subtrees(node_id, prev_id);

            // Now node_id is the leader of the group with same weight.
            node_id = prev_id;
            parent_id = self.nodes[node_id as usize].parent;
        }

        // Find the first node with weight >= prev_weight + 2.
        let target_weight = weight + 2;
        while prev_weight < target_weight {
            prev_id -= 1;
            prev_weight = self.nodes[prev_id as usize].weight;
        }
        prev_id += 1;

        // Swap with leader.
        if prev_id < node_id {
            assert!(self.nodes[node_id as usize].parent < prev_id);
            self.swap_subtrees(node_id, prev_id);
            node_id = prev_id;
        }

        if weight & 1 == 1 {
            // Node is an internal node.
            parent_id
        } else {
            // Node is a leaf node.
            self.nodes[node_id as usize].parent
        }
    }

    // Swap two subtrees. Parents are unchanged.
    fn swap_subtrees(&mut self, node1_id: u16, node2_id: u16) {
        if DEBUG {
            LOG.print(&format!("Swap symbols: {} <-> {}", node1_id, node2_id));
        }

        let node1_copy = self.nodes[node1_id as usize].clone();
        let node2_copy = self.nodes[node2_id as usize].clone();

        fn update(nodes: &mut [Node], num_symbols: u16, node_id: u16, from: Node) {
            let node = &mut nodes[node_id as usize];
            node.weight = from.weight;
            node.child = from.child;
            nodes[from.child as usize].parent = node_id;

            // If `from` is not a leaf node, update the parent reference of the right child as well.
            if from.child > num_symbols {
                nodes[(from.child + 1) as usize].parent = node_id;
            }
        }
        update(&mut self.nodes, self.num_symbols, node1_id, node2_copy);
        update(&mut self.nodes, self.num_symbols, node2_id, node1_copy);
    }

    // Output the code for the given node.
    fn output_code(&mut self, mut node_id: u16, bit_writer: &mut BitWriter) {
        let mut code = 0u64;
        let mut bit = 1u64;
        let mut len: u32 = 0;
        let mut parent_id = self.nodes[node_id as usize].parent;

        loop {
            let parent_node = &self.nodes[parent_id as usize];

            // Output a bit if the node is the right child.
            if parent_node.child != node_id {
                code |= bit;
            }
            len += 1;

            node_id = parent_id;
            parent_id = parent_node.parent;
            if parent_id == 0 {
                break;
            }
            bit <<= 1;

            if len == 64 {
                bit_writer.write_bits(code, len);
                code = 0;
                bit = 1;
                len = 0;
            }
        }
        LOG.print(&format!("Output code:{:#x} len:{}", code, len));
        bit_writer.write_bits(code, len);
    }

    // Output the raw symbol.
    fn output_raw_symbol(&mut self, symbol: u16, bit_writer: &mut BitWriter) {
        if DEBUG {
            LOG.print(&format!("Output raw symbol: {}", symbol));
        }
        bit_writer.write_bits(symbol as u64, self.symbol_bits);
    }

    // Print the nodes and the tree structure.
    pub fn print(&self, title: &str) {
        LOG.print(&format!("= {} =", title));
        if DEBUG_SORTED_NODES {
            self.print_nodes_linear();
        }
        self.print_tree();
    }

    // Print the nodes in linear order.
    fn print_nodes_linear(&self) {
        LOG.print("Nodes: ");
        let mut output = String::new();
        for (i, node) in self.nodes[self.root_node_id() as usize..]
            .iter()
            .enumerate()
        {
            output.push_str(&format!(
                "[{}: weight:{} parent:{} {}]",
                i, node.weight, node.parent, node.child
            ));
        }
        LOG.print(&output);
    }

    // Print the tree structure.
    fn print_tree(&self) {
        LOG.print("Tree: ");
        self.print_node(self.root_node_id(), 0);
    }

    // Print a node with a given indentation (for tree structure).
    fn print_node(&self, node_id: u16, indent: usize) {
        fn indent_str(indent: usize) -> String {
            let mut output = String::new();
            for _ in 0..indent {
                output.push_str("| ");
            }
            output
        }

        let node = &self.nodes[node_id as usize];
        LOG.print(&format!(
            "{}{}, weight:{} parent:{} {}",
            indent_str(indent),
            node_id,
            node.weight,
            node.parent,
            node.child
        ));
        if node.child > self.num_symbols {
            // Is an internal node.
            self.print_node(node.child, indent + 1);
            self.print_node(node.child + 1, indent + 1);
        }
    }

    // Validate the tree structure.
    pub fn validate(&self) {
        // Validate from the root node downwards.
        self.validate_node(self.root_node_id());

        // Validate the nodes are sorted by weight.
        for i in self.root_node_id() as usize..self.nodes.len() - 2 {
            assert!(self.nodes[i].weight >= self.nodes[i + 1].weight);
        }
    }

    // Validate the node invariants.
    fn validate_node(&self, node_id: u16) {
        assert!(node_id > self.num_symbols);

        let node = &self.nodes[node_id as usize];
        let child_id = node.child;

        // Validate NYT node.
        if node_id == (self.nodes.len() - 1) as u16 {
            assert_eq!(child_id, NYT_SYMBOL);
            assert_eq!(node.weight, 0);
            return;
        }

        // Validate parent and children relationships.
        let left_child = &self.nodes[child_id as usize];
        assert_eq!(left_child.parent, node_id);

        if child_id > self.num_symbols {
            // Validate internal nodes.
            assert_eq!(node.weight & 1, 1);

            // Validate right child.
            let right_child = &self.nodes[(child_id + 1) as usize];
            assert_eq!(right_child.parent, node_id);

            // Children weight add up to the parent weight (ignoring the lsb).
            assert_eq!(
                node.weight,
                (((left_child.weight >> 1) + (right_child.weight >> 1)) << 1) + 1
            );

            // Internal nodes have lsb set to 1.
            assert!(node.weight & 1 == 1);
            self.validate_node(child_id);
            self.validate_node(child_id + 1);
        } else {
            // Validate leaf nodes.
            assert_eq!(node.weight & 1, 0);

            // Validate symbol nodes.
            assert_eq!(left_child.weight, WeightType::MAX);
            assert_eq!(left_child.child, NYT_SYMBOL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs, Rng, SeedableRng};

    // Encode a symbol and validate the tree.
    fn encode(huffman: &mut DynamicHuffman, symbol: u16, writer: &mut BitWriter) {
        huffman.encode(symbol, writer);
        if DEBUG_TREE {
            huffman.print("After encode");
        }
        huffman.validate();
    }

    // Decode a symbol and validate the tree.
    fn decode(
        huffman: &mut DynamicHuffman,
        expected_symbol: u16,
        reader: &mut BitReader,
    ) {
        let symbol = huffman.decode(reader);
        if DEBUG_TREE {
            huffman.print("After decode");
        }
        huffman.validate();
        assert_eq!(symbol, expected_symbol);
    }

    #[test]
    fn test_simple() {
        let mut cursor = io::Cursor::new(Vec::new());
        let mut writer = BitWriter::new(&mut cursor);
        let mut huffman = DynamicHuffman::new(20);
        for i in 0..5 {
            encode(&mut huffman, i, &mut writer);
        }
        for i in 0..5 {
            encode(&mut huffman, i, &mut writer);
        }
        for _ in 0..10 {
            encode(&mut huffman, 0, &mut writer);
        }
        writer.finish();
    }

    #[test]
    fn test_random() {
        for s in 0..100 {
            let mut rng = rngs::SmallRng::seed_from_u64(s);
            let mut cursor = io::Cursor::new(Vec::new());
            let mut writer = BitWriter::new(&mut cursor);
            let mut huffman = DynamicHuffman::new(256);

            for _ in 0..500 {
                let symbol = rng.gen::<u8>() as u16;
                encode(&mut huffman, symbol, &mut writer);
            }
            writer.finish();
        }
    }

    #[test]
    fn test_encode_decode() {
        // Encode
        let mut encode_cursor = io::Cursor::new(Vec::new());
        let mut writer = BitWriter::new(&mut encode_cursor);
        let mut huffman = DynamicHuffman::new(20);
        for i in 0..20 {
            encode(&mut huffman, i, &mut writer);
        }
        for i in 0..20 {    
            encode(&mut huffman, i, &mut writer);
        }
        writer.finish();
        assert_eq!(writer.num_write_errors(), 0);

        // Decode
        let mut decode_cursor = io::Cursor::new(encode_cursor.into_inner());
        let mut reader = BitReader::new(&mut decode_cursor);
        let mut huffman = DynamicHuffman::new(20);

        for i in 0..20 {
            decode(&mut huffman, i, &mut reader);
        }
        for i in 0..20 {
            decode(&mut huffman, i, &mut reader);
        }
    }
}
