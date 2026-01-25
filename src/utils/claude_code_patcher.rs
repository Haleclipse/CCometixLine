use std::fs;
use std::path::Path;
use tree_sitter::{Node, Parser, Tree};

#[derive(Debug, Clone)]
pub struct LocationResult {
    pub start_index: usize,
    pub end_index: usize,
    pub variable_name: Option<String>,
}

/// Information about a patch to be applied
#[derive(Debug)]
struct PatchInfo {
    location: LocationResult,
    replacement: String,
}

#[derive(Debug)]
pub struct ClaudeCodePatcher {
    file_content: String,
    file_path: String,
}

impl ClaudeCodePatcher {
    pub fn new<P: AsRef<Path>>(file_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path = file_path.as_ref();
        let content = fs::read_to_string(path)?;

        Ok(Self {
            file_content: content,
            file_path: path.to_string_lossy().to_string(),
        })
    }

    /// Get the version of Claude Code from the file header
    /// Format: // Version: X.Y.Z
    pub fn get_version(&self) -> Option<(u32, u32, u32)> {
        // Look for "// Version: X.Y.Z" in the first 500 bytes
        let header = &self.file_content[..std::cmp::min(500, self.file_content.len())];

        for line in header.lines() {
            if line.starts_with("// Version:") {
                let version_str = line.trim_start_matches("// Version:").trim();
                let parts: Vec<&str> = version_str.split('.').collect();
                if parts.len() >= 3 {
                    let major = parts[0].parse().ok()?;
                    let minor = parts[1].parse().ok()?;
                    let patch = parts[2].parse().ok()?;
                    return Some((major, minor, patch));
                }
            }
        }
        None
    }

    /// Check if version is >= the specified version
    pub fn version_gte(&self, major: u32, minor: u32, patch: u32) -> bool {
        if let Some((v_major, v_minor, v_patch)) = self.get_version() {
            if v_major > major {
                return true;
            }
            if v_major == major && v_minor > minor {
                return true;
            }
            if v_major == major && v_minor == minor && v_patch >= patch {
                return true;
            }
        }
        false
    }

    // =========================================================================
    // Core parsing - parse once and reuse
    // =========================================================================

    /// Parse the file content into an AST tree (called once)
    fn parse_tree(&self) -> Option<Tree> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .ok()?;
        parser.parse(&self.file_content, None)
    }

    /// Get the text content of a node
    fn get_node_text(&self, node: Node) -> String {
        self.file_content[node.start_byte()..node.end_byte()].to_string()
    }

    // =========================================================================
    // Patch 1: Spinner Token Counter (verbose property)
    // Enables detailed token usage display: "1m 38s · ↑ 2.8k tokens"
    // =========================================================================

    /// Find the Spinner verbose property location using cached AST
    fn find_spinner_verbose_property(&self, root: Node) -> Option<LocationResult> {
        self.find_spinner_verbose_in_node(root)
    }

    /// Recursively search for Spinner verbose property in createElement calls
    /// Uses depth-first search but checks children BEFORE parent to find the most specific match
    fn find_spinner_verbose_in_node(&self, node: Node) -> Option<LocationResult> {
        // First, recursively search children (depth-first, children before parent)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_spinner_verbose_in_node(child) {
                return Some(result);
            }
        }

        // Then check if this node is the target createElement call
        if node.kind() == "call_expression" {
            if let Some(result) = self.check_spinner_verbose_call(node) {
                return Some(result);
            }
        }

        None
    }

    /// Check if a call_expression is the Spinner createElement with verbose property
    fn check_spinner_verbose_call(&self, node: Node) -> Option<LocationResult> {
        let function = node.child_by_field_name("function")?;
        let function_text = self.get_node_text(function);

        if !function_text.ends_with("createElement") && function_text != "createElement" {
            return None;
        }

        let arguments = node.child_by_field_name("arguments")?;
        let props_object = self.get_nth_argument(arguments, 1)?;

        if props_object.kind() != "object" {
            return None;
        }

        let has_spinner_tip = self.object_has_direct_key(props_object, "spinnerTip");
        let has_override_message = self.object_has_direct_key(props_object, "overrideMessage");

        if !has_spinner_tip || !has_override_message {
            return None;
        }

        println!(
            "Found Spinner component with spinnerTip and overrideMessage at {}-{}",
            node.start_byte(),
            node.end_byte()
        );

        self.find_spinner_verbose_in_object(props_object)
    }

    fn get_nth_argument<'a>(&self, arguments: Node<'a>, index: usize) -> Option<Node<'a>> {
        let mut cursor = arguments.walk();
        let mut current_index = 0;

        for child in arguments.children(&mut cursor) {
            if child.kind() == "(" || child.kind() == ")" || child.kind() == "," {
                continue;
            }
            if current_index == index {
                return Some(child);
            }
            current_index += 1;
        }
        None
    }

    fn object_has_direct_key(&self, object: Node, key_name: &str) -> bool {
        let mut cursor = object.walk();
        for child in object.children(&mut cursor) {
            if child.kind() == "pair" {
                if let Some(key) = child.child_by_field_name("key") {
                    if self.get_node_text(key) == key_name {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn find_spinner_verbose_in_object(&self, object: Node) -> Option<LocationResult> {
        let mut cursor = object.walk();
        for child in object.children(&mut cursor) {
            if child.kind() == "pair" {
                if let Some(key) = child.child_by_field_name("key") {
                    if self.get_node_text(key) == "verbose" {
                        let start = child.start_byte();
                        let end = child.end_byte();
                        let text = self.get_node_text(child);

                        println!(
                            "Found Spinner verbose property: '{}' at {}-{}",
                            text, start, end
                        );

                        return Some(LocationResult {
                            start_index: start,
                            end_index: end,
                            variable_name: Some(text),
                        });
                    }
                }
            }
        }
        None
    }

    // =========================================================================
    // Patch 2: Context Low Warnings
    // =========================================================================

    /// Find context low condition using cached AST
    fn find_context_low_condition(&self, root: Node) -> Option<LocationResult> {
        self.find_context_low_if_statement(root)
    }

    fn find_context_low_if_statement(&self, node: Node) -> Option<LocationResult> {
        if node.kind() == "function_declaration" || node.kind() == "function" {
            let node_text = self.get_node_text(node);

            if node_text.contains("Context low (") {
                println!(
                    "Found context low function at {}-{}",
                    node.start_byte(),
                    node.end_byte()
                );
                return self.find_if_return_null_in_function(node);
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_context_low_if_statement(child) {
                return Some(result);
            }
        }
        None
    }

    fn find_if_return_null_in_function(&self, node: Node) -> Option<LocationResult> {
        if node.kind() == "if_statement" {
            let node_text = self.get_node_text(node);

            if node_text.contains("return null") && !node_text.contains("else") {
                let consequence = node.child_by_field_name("consequence")?;
                let consequence_text = self.get_node_text(consequence);

                if consequence_text.trim() == "return null"
                    || consequence_text.contains("return null;")
                {
                    let start = node.start_byte();
                    let end = node.end_byte();

                    println!(
                        "Found if statement: '{}' at {}-{}",
                        node_text.trim(),
                        start,
                        end
                    );

                    return Some(LocationResult {
                        start_index: start,
                        end_index: end,
                        variable_name: Some(node_text),
                    });
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_if_return_null_in_function(child) {
                return Some(result);
            }
        }
        None
    }

    // =========================================================================
    // Patch 3: ESC Interrupt Display
    // =========================================================================

    /// Find ESC interrupt condition using cached AST
    fn find_esc_interrupt_condition(&self, root: Node) -> Option<LocationResult> {
        println!("Parsing JavaScript with tree-sitter...");

        let result = self.find_esc_ternary_in_node(root);

        if result.is_some() {
            println!("  ✅ Found ESC interrupt ternary via AST");
        } else {
            println!("  ❌ Could not find ESC interrupt ternary in AST");
        }

        result
    }

    fn find_esc_ternary_in_node(&self, node: Node) -> Option<LocationResult> {
        if node.kind() == "ternary_expression" {
            if let Some(result) = self.check_esc_ternary(node) {
                return Some(result);
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_esc_ternary_in_node(child) {
                return Some(result);
            }
        }
        None
    }

    fn check_esc_ternary(&self, node: Node) -> Option<LocationResult> {
        let condition = node.child_by_field_name("condition")?;
        let consequence = node.child_by_field_name("consequence")?;
        let alternative = node.child_by_field_name("alternative")?;

        let consequence_text = self.get_node_text(consequence);
        let alternative_text = self.get_node_text(alternative);

        if !consequence_text.contains(r#"key:"esc""#) {
            return None;
        }

        if alternative_text.trim() != "[]" {
            return None;
        }

        let condition_start = condition.start_byte();
        let condition_end = condition.end_byte();
        let condition_text = self.get_node_text(condition);

        println!(
            "  Found ESC ternary: condition='{}' at {}-{}",
            condition_text, condition_start, condition_end
        );
        println!(
            "    consequence contains key:\"esc\": {}",
            consequence_text.len() > 50
        );
        println!(
            "    alternative is empty array: {}",
            alternative_text == "[]"
        );

        Some(LocationResult {
            start_index: condition_start,
            end_index: condition_end,
            variable_name: Some(condition_text),
        })
    }

    // =========================================================================
    // Patch 4: Chrome Subscription Check
    // =========================================================================

    /// Find Chrome subscription check using cached AST and anchor
    fn find_chrome_subscription_check(&self, root: Node) -> Option<LocationResult> {
        let anchor = "tengu_claude_in_chrome_setup";
        let anchor_pos = self.file_content.find(anchor)?;
        println!("Found anchor '{}' at position: {}", anchor, anchor_pos);

        self.find_chrome_check_in_node(root, anchor_pos)
    }

    fn find_chrome_check_in_node(&self, node: Node, anchor_pos: usize) -> Option<LocationResult> {
        if (node.kind() == "lexical_declaration" || node.kind() == "variable_declaration")
            && node.end_byte() < anchor_pos
            && anchor_pos - node.end_byte() < 300
        {
            if let Some(result) = self.check_chrome_declaration(node) {
                return Some(result);
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_chrome_check_in_node(child, anchor_pos) {
                return Some(result);
            }
        }
        None
    }

    fn check_chrome_declaration(&self, node: Node) -> Option<LocationResult> {
        let node_text = self.get_node_text(node);

        if !node_text.contains(".chrome") || !node_text.contains("&&") {
            return None;
        }

        println!("Found Chrome check pattern: '{}'", node_text);
        self.find_and_expression_in_node(node)
    }

    fn find_and_expression_in_node(&self, node: Node) -> Option<LocationResult> {
        if node.kind() == "binary_expression" {
            let node_text = self.get_node_text(node);
            if node_text.contains("&&") {
                let left = node.child_by_field_name("left")?;
                let left_text = self.get_node_text(left);

                if left_text.contains(".chrome") {
                    let right = node.child_by_field_name("right")?;
                    let and_start = left.end_byte();
                    let and_end = right.end_byte();
                    let and_text = self.file_content[and_start..and_end].to_string();

                    println!(
                        "Part to remove: '{}' at {}-{}",
                        and_text, and_start, and_end
                    );

                    return Some(LocationResult {
                        start_index: and_start,
                        end_index: and_end,
                        variable_name: Some(and_text),
                    });
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_and_expression_in_node(child) {
                return Some(result);
            }
        }
        None
    }

    // =========================================================================
    // Patch 5: /chrome Command Message
    // =========================================================================

    /// Find /chrome command message using cached AST and anchor
    fn find_chrome_command_message(&self, root: Node) -> Option<LocationResult> {
        let anchor = r#""Claude in Chrome requires a claude.ai subscription.""#;
        let anchor_pos = self.file_content.find(anchor)?;
        println!(
            "Found /chrome subscription message at position: {}",
            anchor_pos
        );

        self.find_chrome_message_condition(root, anchor_pos)
    }

    fn find_chrome_message_condition(
        &self,
        node: Node,
        anchor_pos: usize,
    ) -> Option<LocationResult> {
        if node.kind() == "binary_expression"
            && node.start_byte() < anchor_pos
            && anchor_pos - node.start_byte() < 100
        {
            if let Some(result) = self.check_not_and_expression(node, anchor_pos) {
                return Some(result);
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_chrome_message_condition(child, anchor_pos) {
                return Some(result);
            }
        }
        None
    }

    fn check_not_and_expression(&self, node: Node, anchor_pos: usize) -> Option<LocationResult> {
        let left = node.child_by_field_name("left")?;
        let operator = node.child_by_field_name("operator")?;

        if self.get_node_text(operator) != "&&" {
            return None;
        }

        if left.kind() != "unary_expression" {
            return None;
        }

        let left_text = self.get_node_text(left);
        if !left_text.starts_with("!") {
            return None;
        }

        let node_start = node.start_byte();
        let node_end = node.end_byte();

        if anchor_pos >= node_start && anchor_pos <= node_end {
            let op_end = operator.end_byte();
            let replace_start = left.start_byte();
            let replace_end = op_end;
            let replace_text = self.file_content[replace_start..replace_end].to_string();

            println!(
                "  Found condition '{}' at {}-{}",
                replace_text, replace_start, replace_end
            );

            return Some(LocationResult {
                start_index: replace_start,
                end_index: replace_end,
                variable_name: Some(replace_text),
            });
        }

        None
    }

    // =========================================================================
    // Patch 6: Chrome Startup Notification
    // =========================================================================

    /// Find Chrome startup notification using cached AST and anchor
    fn find_chrome_startup_notification_check(&self, root: Node) -> Option<LocationResult> {
        let anchor = r#"key:"chrome-requires-subscription""#;
        let anchor_pos = self.file_content.find(anchor)?;
        println!(
            "Found Chrome startup notification anchor at position: {}",
            anchor_pos
        );

        self.find_startup_notification_if(root, anchor_pos)
    }

    fn find_startup_notification_if(
        &self,
        node: Node,
        anchor_pos: usize,
    ) -> Option<LocationResult> {
        if node.kind() == "if_statement"
            && node.start_byte() < anchor_pos
            && anchor_pos - node.start_byte() < 150
        {
            let node_text = self.get_node_text(node);
            if node_text.contains("chrome-requires-subscription") {
                if let Some(result) = self.check_startup_notification_condition(node) {
                    return Some(result);
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_startup_notification_if(child, anchor_pos) {
                return Some(result);
            }
        }
        None
    }

    fn check_startup_notification_condition(&self, node: Node) -> Option<LocationResult> {
        let condition = node.child_by_field_name("condition")?;

        if condition.kind() == "parenthesized_expression" {
            let mut cursor = condition.walk();
            for child in condition.children(&mut cursor) {
                if child.kind() == "unary_expression" {
                    let child_text = self.get_node_text(child);
                    if child_text.starts_with("!") && child_text.contains("()") {
                        let start = child.start_byte();
                        let end = child.end_byte();

                        println!("  Found condition '{}' at {}-{}", child_text, start, end);

                        return Some(LocationResult {
                            start_index: start,
                            end_index: end,
                            variable_name: Some(child_text),
                        });
                    }
                }
            }
        }

        None
    }

    // =========================================================================
    // Utility functions
    // =========================================================================

    /// Show a diff of the changes (for debugging)
    fn show_diff(&self, title: &str, injected_text: &str, start_index: usize, end_index: usize) {
        let context_start = start_index.saturating_sub(50);
        let context_end_old = std::cmp::min(self.file_content.len(), end_index + 50);

        let old_before = &self.file_content[context_start..start_index];
        let old_changed = &self.file_content[start_index..end_index];
        let old_after = &self.file_content[end_index..context_end_old];

        println!("\n--- {} Diff ---", title);
        println!(
            "OLD: {}\x1b[31m{}\x1b[0m{}",
            old_before, old_changed, old_after
        );
        println!(
            "NEW: {}\x1b[32m{}\x1b[0m{}",
            old_before, injected_text, old_after
        );
        println!("--- End Diff ---\n");
    }

    /// Save the modified content back to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(&self.file_path, &self.file_content)?;
        Ok(())
    }

    /// Get a reference to the file content (for testing purposes)
    pub fn get_file_content(&self) -> &str {
        &self.file_content
    }

    // =========================================================================
    // Optimized batch patching - parse once, apply all
    // =========================================================================

    /// Apply all patches using optimized single-parse strategy
    /// Returns results in the same order as the original implementation
    pub fn apply_all_patches(&mut self) -> Vec<(&'static str, bool)> {
        let mut results = Vec::new();

        // Parse AST only once
        let tree = match self.parse_tree() {
            Some(t) => t,
            None => {
                println!("⚠️ Failed to parse JavaScript AST");
                return vec![
                    ("Spinner token counter", false),
                    ("Context low warnings", false),
                    ("ESC interrupt display", false),
                    ("Chrome subscription check", false),
                    ("/chrome command message", false),
                    ("Chrome startup notification", false),
                ];
            }
        };

        let root = tree.root_node();
        let mut patches: Vec<PatchInfo> = Vec::new();

        // 1. Spinner token counter (verbose property)
        match self.find_spinner_verbose_property(root) {
            Some(loc) => {
                let replacement = "verbose:true".to_string();
                self.show_diff(
                    "Spinner Token Counter",
                    &replacement,
                    loc.start_index,
                    loc.end_index,
                );
                patches.push(PatchInfo {
                    location: loc,
                    replacement,
                });
                results.push(("Spinner token counter", true));
            }
            None => {
                println!("⚠️ Could not enable Spinner token counter");
                results.push(("Spinner token counter", false));
            }
        }

        // 2. Context low warnings
        match self.find_context_low_condition(root) {
            Some(loc) => {
                let replacement = "if(true)return null;".to_string();
                self.show_diff(
                    "Context Low Condition",
                    &replacement,
                    loc.start_index,
                    loc.end_index,
                );
                patches.push(PatchInfo {
                    location: loc,
                    replacement,
                });
                results.push(("Context low warnings", true));
            }
            None => {
                println!("⚠️ Could not disable context low warnings");
                results.push(("Context low warnings", false));
            }
        }

        // 3. ESC interrupt display
        match self.find_esc_interrupt_condition(root) {
            Some(loc) => {
                let original_condition = loc.variable_name.clone().unwrap_or_default();
                println!(
                    "Replacing condition '{}' with '(false)' at position {}-{}",
                    original_condition, loc.start_index, loc.end_index
                );
                let replacement = "(false)".to_string();
                self.show_diff(
                    "ESC Interrupt",
                    &replacement,
                    loc.start_index,
                    loc.end_index,
                );
                patches.push(PatchInfo {
                    location: loc,
                    replacement,
                });
                results.push(("ESC interrupt display", true));
            }
            None => {
                println!("⚠️ Could not disable esc/interrupt display");
                results.push(("ESC interrupt display", false));
            }
        }

        // 4. Chrome subscription check
        match self.find_chrome_subscription_check(root) {
            Some(loc) => {
                println!(
                    "Removing '{}' at position {}-{}",
                    loc.variable_name.as_ref().unwrap_or(&String::new()),
                    loc.start_index,
                    loc.end_index
                );
                let replacement = "".to_string();
                self.show_diff(
                    "Chrome Subscription Check",
                    &replacement,
                    loc.start_index,
                    loc.end_index,
                );
                patches.push(PatchInfo {
                    location: loc,
                    replacement,
                });
                results.push(("Chrome subscription check", true));
            }
            None => {
                println!("⚠️ Could not bypass Chrome subscription check");
                results.push(("Chrome subscription check", false));
            }
        }

        // 5. /chrome command message
        match self.find_chrome_command_message(root) {
            Some(loc) => {
                println!(
                    "Replacing '{}' with 'false&&' at position {}-{}",
                    loc.variable_name.as_ref().unwrap_or(&String::new()),
                    loc.start_index,
                    loc.end_index
                );
                let replacement = "false&&".to_string();
                self.show_diff(
                    "/chrome Command Message",
                    &replacement,
                    loc.start_index,
                    loc.end_index,
                );
                patches.push(PatchInfo {
                    location: loc,
                    replacement,
                });
                results.push(("/chrome command message", true));
            }
            None => {
                println!("⚠️ Could not remove /chrome command subscription message");
                results.push(("/chrome command message", false));
            }
        }

        // 6. Chrome startup notification
        match self.find_chrome_startup_notification_check(root) {
            Some(loc) => {
                println!(
                    "Replacing '{}' with 'false' at position {}-{}",
                    loc.variable_name.as_ref().unwrap_or(&String::new()),
                    loc.start_index,
                    loc.end_index
                );
                let replacement = "false".to_string();
                self.show_diff(
                    "Chrome Startup Notification",
                    &replacement,
                    loc.start_index,
                    loc.end_index,
                );
                patches.push(PatchInfo {
                    location: loc,
                    replacement,
                });
                results.push(("Chrome startup notification", true));
            }
            None => {
                println!("⚠️ Could not remove Chrome startup notification check");
                results.push(("Chrome startup notification", false));
            }
        }

        // Sort patches by position descending (apply from end to start to avoid offset issues)
        patches.sort_by(|a, b| b.location.start_index.cmp(&a.location.start_index));

        // Apply all patches in one pass
        for patch in patches {
            let new_content = format!(
                "{}{}{}",
                &self.file_content[..patch.location.start_index],
                patch.replacement,
                &self.file_content[patch.location.end_index..]
            );
            self.file_content = new_content;
        }

        results
    }

    /// Print patch results summary
    pub fn print_summary(results: &[(&str, bool)]) {
        println!("\n📊 Patch Results:");
        for (name, success) in results {
            if *success {
                println!("  ✅ {}", name);
            } else {
                println!("  ❌ {}", name);
            }
        }

        let success_count = results.iter().filter(|(_, s)| *s).count();
        let total_count = results.len();

        if success_count == total_count {
            println!("\n✅ All {} patches applied successfully!", total_count);
        } else {
            println!(
                "\n⚠️ {}/{} patches applied successfully",
                success_count, total_count
            );
        }
    }
}
