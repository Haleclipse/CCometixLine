use std::fs;
use std::path::Path;
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone)]
pub struct LocationResult {
    pub start_index: usize,
    pub end_index: usize,
    pub variable_name: Option<String>,
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

    /// Find the verbose property location using tree-sitter AST
    /// Searches for createElement call with spinnerTip and overrideMessage, then finds verbose property
    pub fn get_verbose_property_location(&self) -> Option<LocationResult> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .expect("Error loading JavaScript grammar");

        let tree = parser.parse(&self.file_content, None)?;
        let root_node = tree.root_node();

        // Find createElement call containing spinnerTip and overrideMessage
        self.find_verbose_property_in_node(root_node)
    }

    /// Recursively search for verbose property in createElement calls
    fn find_verbose_property_in_node(&self, node: Node) -> Option<LocationResult> {
        // Check if this is a call_expression (function call)
        if node.kind() == "call_expression" {
            if let Some(result) = self.check_verbose_call(node) {
                return Some(result);
            }
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_verbose_property_in_node(child) {
                return Some(result);
            }
        }

        None
    }

    /// Check if a call_expression is the createElement with verbose property
    fn check_verbose_call(&self, node: Node) -> Option<LocationResult> {
        let node_text = self.get_node_text(node);

        // Must be a createElement call with spinnerTip and overrideMessage
        if !node_text.contains("createElement")
            || !node_text.contains("spinnerTip")
            || !node_text.contains("overrideMessage")
        {
            return None;
        }

        // Find the arguments node (second argument should be the props object)
        let arguments = node.child_by_field_name("arguments")?;

        // Search for verbose property in the arguments
        self.find_verbose_in_arguments(arguments)
    }

    /// Find verbose property within arguments
    fn find_verbose_in_arguments(&self, node: Node) -> Option<LocationResult> {
        // Look for pair nodes with key "verbose"
        if node.kind() == "pair" {
            let key = node.child_by_field_name("key")?;
            let key_text = self.get_node_text(key);

            if key_text == "verbose" {
                let start = node.start_byte();
                let end = node.end_byte();
                let text = self.get_node_text(node);

                println!("Found verbose property: '{}' at {}-{}", text, start, end);

                return Some(LocationResult {
                    start_index: start,
                    end_index: end,
                    variable_name: Some(text),
                });
            }
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_verbose_in_arguments(child) {
                return Some(result);
            }
        }

        None
    }

    /// Write the verbose property with new value
    pub fn write_verbose_property(
        &mut self,
        value: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let location = self
            .get_verbose_property_location()
            .ok_or("Failed to find verbose property location")?;

        let new_code = format!("verbose:{}", value);

        let new_content = format!(
            "{}{}{}",
            &self.file_content[..location.start_index],
            new_code,
            &self.file_content[location.end_index..]
        );

        self.show_diff(
            "Verbose Property",
            &new_code,
            location.start_index,
            location.end_index,
        );
        self.file_content = new_content;

        Ok(())
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

    /// Find context low condition using tree-sitter AST
    /// Searches for function containing "Context low (" and finds the if(...)return null statement
    pub fn get_context_low_condition_location(&self) -> Option<LocationResult> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .expect("Error loading JavaScript grammar");

        let tree = parser.parse(&self.file_content, None)?;
        let root_node = tree.root_node();

        // Find the function containing "Context low ("
        self.find_context_low_if_statement(root_node)
    }

    /// Recursively search for the context low function and its if statement
    fn find_context_low_if_statement(&self, node: Node) -> Option<LocationResult> {
        // Check if this is a function declaration or function expression
        if node.kind() == "function_declaration" || node.kind() == "function" {
            let node_text = self.get_node_text(node);

            // Check if this function contains "Context low ("
            if node_text.contains("Context low (") {
                println!(
                    "Found context low function at {}-{}",
                    node.start_byte(),
                    node.end_byte()
                );

                // Find the if statement that returns null
                return self.find_if_return_null_in_function(node);
            }
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_context_low_if_statement(child) {
                return Some(result);
            }
        }

        None
    }

    /// Find if(...)return null statement within a function
    fn find_if_return_null_in_function(&self, node: Node) -> Option<LocationResult> {
        // Check if this is an if_statement
        if node.kind() == "if_statement" {
            let node_text = self.get_node_text(node);

            // Check if this if statement returns null (without else branch)
            if node_text.contains("return null") && !node_text.contains("else") {
                // Get the consequence (the body of the if)
                let consequence = node.child_by_field_name("consequence")?;
                let consequence_text = self.get_node_text(consequence);

                // Make sure this is a simple return null statement
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

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_if_return_null_in_function(child) {
                return Some(result);
            }
        }

        None
    }

    /// Disable context low warnings by modifying the if condition to always return null
    /// Uses tree-sitter AST to find the if statement
    pub fn disable_context_low_warnings(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(location) = self.get_context_low_condition_location() {
            let replacement_condition = "if(true)return null;";

            let new_content = format!(
                "{}{}{}",
                &self.file_content[..location.start_index],
                replacement_condition,
                &self.file_content[location.end_index..]
            );

            self.show_diff(
                "Context Low Condition",
                replacement_condition,
                location.start_index,
                location.end_index,
            );
            self.file_content = new_content;

            Ok(())
        } else {
            Err("Could not locate context low condition using tree-sitter".into())
        }
    }

    /// Find the ternary condition for esc/interrupt display using tree-sitter AST
    /// Searches for: VAR?[...{key:"esc"}...]:[] or ...VAR?[...{key:"esc"}...]:[]
    /// Returns the position of VAR that needs to be replaced with (false)
    fn find_esc_interrupt_condition(&self) -> Option<LocationResult> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .expect("Error loading JavaScript grammar");

        let tree = parser.parse(&self.file_content, None)?;
        let root_node = tree.root_node();

        println!("Parsing JavaScript with tree-sitter...");

        // Find all ternary expressions that contain key:"esc"
        let result = self.find_esc_ternary_in_node(root_node);

        if result.is_some() {
            println!("  ✅ Found ESC interrupt ternary via AST");
        } else {
            println!("  ❌ Could not find ESC interrupt ternary in AST");
        }

        result
    }

    /// Recursively search for the ESC interrupt ternary expression in AST
    fn find_esc_ternary_in_node(&self, node: Node) -> Option<LocationResult> {
        // Check if this is a ternary expression (conditional_expression in tree-sitter-javascript)
        if node.kind() == "ternary_expression" {
            if let Some(result) = self.check_esc_ternary(node) {
                return Some(result);
            }
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_esc_ternary_in_node(child) {
                return Some(result);
            }
        }

        None
    }

    /// Check if a ternary expression is the ESC interrupt pattern
    /// Pattern: CONDITION?[...{key:"esc"}...]:[]
    fn check_esc_ternary(&self, node: Node) -> Option<LocationResult> {
        // ternary_expression has 3 children: condition, consequence, alternative
        let condition = node.child_by_field_name("condition")?;
        let consequence = node.child_by_field_name("consequence")?;
        let alternative = node.child_by_field_name("alternative")?;

        // Get text for consequence and alternative
        let consequence_text = self.get_node_text(consequence);
        let alternative_text = self.get_node_text(alternative);

        // Check if consequence is an array containing key:"esc"
        if !consequence_text.contains(r#"key:"esc""#) {
            return None;
        }

        // Check if alternative is an empty array
        if alternative_text.trim() != "[]" {
            return None;
        }

        // Found the ESC interrupt ternary!
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

    /// Get the text content of a node
    fn get_node_text(&self, node: Node) -> String {
        self.file_content[node.start_byte()..node.end_byte()].to_string()
    }

    /// Disable "esc to interrupt" display by replacing ternary condition with (false)
    /// Changes: ...H1?[esc elements]:[] → ...(false)?[esc elements]:[]
    pub fn disable_esc_interrupt_display(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let location = self
            .find_esc_interrupt_condition()
            .ok_or("Could not find esc/interrupt ternary condition")?;

        let original_condition = location
            .variable_name
            .as_ref()
            .ok_or("No condition variable found")?;

        println!(
            "Replacing condition '{}' with '(false)' at position {}-{}",
            original_condition, location.start_index, location.end_index
        );

        self.show_diff(
            "ESC Interrupt",
            "(false)",
            location.start_index,
            location.end_index,
        );

        let new_content = format!(
            "{}(false){}",
            &self.file_content[..location.start_index],
            &self.file_content[location.end_index..]
        );

        self.file_content = new_content;

        Ok(())
    }

    /// Find the Claude in Chrome subscription check location using tree-sitter AST
    /// Pattern: let VAR=FUNC(PARAM.chrome)&&FUNC2();
    /// Returns the location of "&&FUNC()" to be removed
    fn find_chrome_subscription_check(&self) -> Option<LocationResult> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .expect("Error loading JavaScript grammar");

        let tree = parser.parse(&self.file_content, None)?;
        let root_node = tree.root_node();

        // Find anchor position first
        let anchor = "tengu_claude_in_chrome_setup";
        let anchor_pos = self.file_content.find(anchor)?;
        println!("Found anchor '{}' at position: {}", anchor, anchor_pos);

        // Search for variable declaration with .chrome and && pattern
        self.find_chrome_check_in_node(root_node, anchor_pos)
    }

    /// Recursively search for Chrome subscription check pattern
    fn find_chrome_check_in_node(&self, node: Node, anchor_pos: usize) -> Option<LocationResult> {
        // Look for lexical_declaration (let/const) or variable_declaration (var)
        if node.kind() == "lexical_declaration" || node.kind() == "variable_declaration" {
            // Must be before the anchor
            if node.end_byte() < anchor_pos && anchor_pos - node.end_byte() < 300 {
                if let Some(result) = self.check_chrome_declaration(node) {
                    return Some(result);
                }
            }
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_chrome_check_in_node(child, anchor_pos) {
                return Some(result);
            }
        }

        None
    }

    /// Check if a variable declaration matches the Chrome check pattern
    fn check_chrome_declaration(&self, node: Node) -> Option<LocationResult> {
        let node_text = self.get_node_text(node);

        // Must contain .chrome and &&
        if !node_text.contains(".chrome") || !node_text.contains("&&") {
            return None;
        }

        println!("Found Chrome check pattern: '{}'", node_text);

        // Find the binary_expression with && operator
        self.find_and_expression_in_node(node)
    }

    /// Find && binary expression and return the right operand location
    fn find_and_expression_in_node(&self, node: Node) -> Option<LocationResult> {
        if node.kind() == "binary_expression" {
            // Check if operator is &&
            let node_text = self.get_node_text(node);
            if node_text.contains("&&") {
                // Get the left operand - must contain .chrome
                let left = node.child_by_field_name("left")?;
                let left_text = self.get_node_text(left);

                if left_text.contains(".chrome") {
                    // Get the right operand position (including &&)
                    let right = node.child_by_field_name("right")?;

                    // The part to remove is from after left to end of right (includes &&)
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

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_and_expression_in_node(child) {
                return Some(result);
            }
        }

        None
    }

    /// Bypass Claude in Chrome subscription check
    /// Changes: let qA=XV1(X.chrome)&&zB(); → let qA=XV1(X.chrome);
    /// This removes the subscription check while keeping the feature flag check
    pub fn bypass_chrome_subscription_check(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let location = self
            .find_chrome_subscription_check()
            .ok_or("Could not find Chrome subscription check pattern")?;

        println!(
            "Removing '{}' at position {}-{}",
            location.variable_name.as_ref().unwrap_or(&String::new()),
            location.start_index,
            location.end_index
        );

        self.show_diff(
            "Chrome Subscription Check",
            "",
            location.start_index,
            location.end_index,
        );

        // Remove "&&FUNC()" by replacing it with empty string
        let new_content = format!(
            "{}{}",
            &self.file_content[..location.start_index],
            &self.file_content[location.end_index..]
        );

        self.file_content = new_content;

        Ok(())
    }

    /// Find the /chrome command subscription message location using tree-sitter AST
    /// Pattern: !G&&...createElement(...,"Claude in Chrome requires a claude.ai subscription.")
    /// Returns the location of "!G&&" to be replaced with "false&&"
    fn find_chrome_command_message(&self) -> Option<LocationResult> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .expect("Error loading JavaScript grammar");

        let tree = parser.parse(&self.file_content, None)?;
        let root_node = tree.root_node();

        // Find anchor position
        let anchor = r#""Claude in Chrome requires a claude.ai subscription.""#;
        let anchor_pos = self.file_content.find(anchor)?;
        println!(
            "Found /chrome subscription message at position: {}",
            anchor_pos
        );

        // Search for binary_expression with && where left is unary !
        self.find_chrome_message_condition(root_node, anchor_pos)
    }

    /// Recursively search for !VAR&& pattern before the anchor
    fn find_chrome_message_condition(
        &self,
        node: Node,
        anchor_pos: usize,
    ) -> Option<LocationResult> {
        // Look for binary_expression with && operator
        if node.kind() == "binary_expression" {
            // Must be before anchor and within range
            if node.start_byte() < anchor_pos && anchor_pos - node.start_byte() < 100 {
                // Check if this is a && expression where left is !VAR
                if let Some(result) = self.check_not_and_expression(node, anchor_pos) {
                    return Some(result);
                }
            }
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_chrome_message_condition(child, anchor_pos) {
                return Some(result);
            }
        }

        None
    }

    /// Check if binary_expression is !VAR&& pattern
    fn check_not_and_expression(&self, node: Node, anchor_pos: usize) -> Option<LocationResult> {
        // Get children
        let left = node.child_by_field_name("left")?;
        let operator = node.child_by_field_name("operator")?;

        // Must be && operator
        if self.get_node_text(operator) != "&&" {
            return None;
        }

        // Left must be unary_expression with ! operator
        if left.kind() != "unary_expression" {
            return None;
        }

        let left_text = self.get_node_text(left);
        if !left_text.starts_with("!") {
            return None;
        }

        // Check if this binary_expression contains the anchor (subscription message)
        // The anchor should be inside this expression (in the right operand)
        let node_start = node.start_byte();
        let node_end = node.end_byte();

        if anchor_pos >= node_start && anchor_pos <= node_end {
            // The part to replace is from start of left (!) to end of &&
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

    /// Remove /chrome command subscription message
    /// Changes: !G&&...("requires subscription") → false&&...("requires subscription")
    /// This prevents the error message from being rendered
    pub fn remove_chrome_command_subscription_message(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let location = self
            .find_chrome_command_message()
            .ok_or("Could not find /chrome command subscription message")?;

        println!(
            "Replacing '{}' with 'false&&' at position {}-{}",
            location.variable_name.as_ref().unwrap_or(&String::new()),
            location.start_index,
            location.end_index
        );

        self.show_diff(
            "/chrome Command Message",
            "false&&",
            location.start_index,
            location.end_index,
        );

        // Replace "!G&&" with "false&&"
        let new_content = format!(
            "{}false&&{}",
            &self.file_content[..location.start_index],
            &self.file_content[location.end_index..]
        );

        self.file_content = new_content;

        Ok(())
    }

    /// Find the Chrome startup notification subscription check using tree-sitter AST
    /// Pattern: if(!zB()){A({key:"chrome-requires-subscription"...
    /// Returns the location of "!zB()" to be replaced with "false"
    fn find_chrome_startup_notification_check(&self) -> Option<LocationResult> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .expect("Error loading JavaScript grammar");

        let tree = parser.parse(&self.file_content, None)?;
        let root_node = tree.root_node();

        // Find anchor position
        let anchor = r#"key:"chrome-requires-subscription""#;
        let anchor_pos = self.file_content.find(anchor)?;
        println!(
            "Found Chrome startup notification anchor at position: {}",
            anchor_pos
        );

        // Search for if statement with !FUNC() condition
        self.find_startup_notification_if(root_node, anchor_pos)
    }

    /// Recursively search for if(!FUNC()) pattern before the anchor
    fn find_startup_notification_if(
        &self,
        node: Node,
        anchor_pos: usize,
    ) -> Option<LocationResult> {
        // Look for if_statement
        if node.kind() == "if_statement" {
            // Must be before anchor and within range
            if node.start_byte() < anchor_pos && anchor_pos - node.start_byte() < 150 {
                // Check if the node text contains the anchor
                let node_text = self.get_node_text(node);
                if node_text.contains("chrome-requires-subscription") {
                    if let Some(result) = self.check_startup_notification_condition(node) {
                        return Some(result);
                    }
                }
            }
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_startup_notification_if(child, anchor_pos) {
                return Some(result);
            }
        }

        None
    }

    /// Check if the if_statement has !FUNC() condition
    fn check_startup_notification_condition(&self, node: Node) -> Option<LocationResult> {
        // Get the condition (parenthesized_expression)
        let condition = node.child_by_field_name("condition")?;

        // Check if condition is !FUNC() (unary expression with call)
        if condition.kind() == "parenthesized_expression" {
            // Get the inner expression
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

    /// Remove Chrome startup subscription notification check
    /// Changes: if(!zB()){...} → if(false){...}
    /// This prevents the startup notification from showing
    pub fn remove_chrome_startup_notification_check(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let location = self
            .find_chrome_startup_notification_check()
            .ok_or("Could not find Chrome startup notification check")?;

        println!(
            "Replacing '{}' with 'false' at position {}-{}",
            location.variable_name.as_ref().unwrap_or(&String::new()),
            location.start_index,
            location.end_index
        );

        self.show_diff(
            "Chrome Startup Notification",
            "false",
            location.start_index,
            location.end_index,
        );

        // Replace "!zB()" with "false"
        let new_content = format!(
            "{}false{}",
            &self.file_content[..location.start_index],
            &self.file_content[location.end_index..]
        );

        self.file_content = new_content;

        Ok(())
    }

    /// Find the npm deprecation warning notification call using tree-sitter AST
    /// Pattern: K({timeoutMs:15000,key:"npm-deprecation-warning",...})
    /// Only exists in v2.1.15+
    /// Returns the location of "K({" to be replaced with "0&&K({"
    fn find_npm_deprecation_warning(&self) -> Option<LocationResult> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .expect("Error loading JavaScript grammar");

        let tree = parser.parse(&self.file_content, None)?;
        let root_node = tree.root_node();

        // Find anchor position
        let anchor = r#"key:"npm-deprecation-warning""#;
        let anchor_pos = self.file_content.find(anchor)?;
        println!(
            "Found npm deprecation warning anchor at position: {}",
            anchor_pos
        );

        // Search for call_expression containing the anchor
        self.find_npm_warning_call(root_node, anchor_pos)
    }

    /// Recursively search for call_expression containing npm-deprecation-warning
    fn find_npm_warning_call(&self, node: Node, anchor_pos: usize) -> Option<LocationResult> {
        // Look for call_expression
        if node.kind() == "call_expression" {
            let node_start = node.start_byte();
            let node_end = node.end_byte();

            // Check if this call contains the anchor
            if anchor_pos >= node_start && anchor_pos <= node_end {
                let node_text = self.get_node_text(node);
                if node_text.contains("npm-deprecation-warning") {
                    // This is the notification call K({...})
                    // We want to prepend "0&&" to disable it
                    println!(
                        "  Found npm deprecation call at {}-{}",
                        node_start, node_end
                    );

                    return Some(LocationResult {
                        start_index: node_start,
                        end_index: node_start, // We're inserting, not replacing
                        variable_name: Some("K({...})".to_string()),
                    });
                }
            }
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_npm_warning_call(child, anchor_pos) {
                return Some(result);
            }
        }

        None
    }

    /// Disable npm deprecation warning notification
    /// Changes: K({...npm-deprecation-warning...}) → 0&&K({...npm-deprecation-warning...})
    /// Only applies to v2.1.15+
    pub fn disable_npm_deprecation_warning(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Check version - only exists in 2.1.15+
        if !self.version_gte(2, 1, 15) {
            println!("  ℹ️ npm deprecation warning not applicable (version < 2.1.15)");
            return Ok(());
        }

        let location = self
            .find_npm_deprecation_warning()
            .ok_or("Could not find npm deprecation warning")?;

        println!(
            "Inserting '0&&' at position {} to disable npm deprecation warning",
            location.start_index
        );

        self.show_diff(
            "npm Deprecation Warning",
            "0&&",
            location.start_index,
            location.start_index,
        );

        // Insert "0&&" before the call
        let new_content = format!(
            "{}0&&{}",
            &self.file_content[..location.start_index],
            &self.file_content[location.start_index..]
        );

        self.file_content = new_content;

        Ok(())
    }

    /// Apply all patches and return results
    pub fn apply_all_patches(&mut self) -> Vec<(&'static str, bool)> {
        let mut results = Vec::new();

        // 1. Set verbose property to true
        match self.write_verbose_property(true) {
            Ok(_) => results.push(("Verbose property", true)),
            Err(e) => {
                println!("⚠️ Could not modify verbose property: {}", e);
                results.push(("Verbose property", false));
            }
        }

        // 2. Disable context low warnings
        match self.disable_context_low_warnings() {
            Ok(_) => results.push(("Context low warnings", true)),
            Err(e) => {
                println!("⚠️ Could not disable context low warnings: {}", e);
                results.push(("Context low warnings", false));
            }
        }

        // 3. Disable ESC interrupt display
        match self.disable_esc_interrupt_display() {
            Ok(_) => results.push(("ESC interrupt display", true)),
            Err(e) => {
                println!("⚠️ Could not disable esc/interrupt display: {}", e);
                results.push(("ESC interrupt display", false));
            }
        }

        // 4. Bypass Chrome subscription check
        match self.bypass_chrome_subscription_check() {
            Ok(_) => results.push(("Chrome subscription check", true)),
            Err(e) => {
                println!("⚠️ Could not bypass Chrome subscription check: {}", e);
                results.push(("Chrome subscription check", false));
            }
        }

        // 5. Remove /chrome command subscription message
        match self.remove_chrome_command_subscription_message() {
            Ok(_) => results.push(("/chrome command message", true)),
            Err(e) => {
                println!(
                    "⚠️ Could not remove /chrome command subscription message: {}",
                    e
                );
                results.push(("/chrome command message", false));
            }
        }

        // 6. Remove Chrome startup notification check
        match self.remove_chrome_startup_notification_check() {
            Ok(_) => results.push(("Chrome startup notification", true)),
            Err(e) => {
                println!(
                    "⚠️ Could not remove Chrome startup notification check: {}",
                    e
                );
                results.push(("Chrome startup notification", false));
            }
        }

        // 7. Disable npm deprecation warning (v2.1.15+ only)
        match self.disable_npm_deprecation_warning() {
            Ok(_) => results.push(("npm deprecation warning", true)),
            Err(e) => {
                println!("⚠️ Could not disable npm deprecation warning: {}", e);
                results.push(("npm deprecation warning", false));
            }
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
