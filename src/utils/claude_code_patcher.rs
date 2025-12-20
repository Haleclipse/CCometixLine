use regex::Regex;
use std::fs;
use std::path::Path;

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

    /// Find the verbose property location in Claude Code's cli.js
    /// Based on the pattern from patching.ts getVerbosePropertyLocation function
    pub fn get_verbose_property_location(&self) -> Option<LocationResult> {
        // Step 1: Find createElement pattern with spinnerTip and overrideMessage
        let create_element_pattern =
            Regex::new(r"createElement\([$\w]+,\{[^}]+spinnerTip[^}]+overrideMessage[^}]+\}")
                .ok()?;

        let create_element_match = create_element_pattern.find(&self.file_content)?;
        let extracted_string =
            &self.file_content[create_element_match.start()..create_element_match.end()];

        println!(
            "Found createElement match at: {}-{}",
            create_element_match.start(),
            create_element_match.end()
        );
        println!(
            "Extracted string: {}",
            &extracted_string[..std::cmp::min(200, extracted_string.len())]
        );

        // Step 2: Find verbose property within the createElement match
        let verbose_pattern = Regex::new(r"verbose:[^,}]+").ok()?;
        let verbose_match = verbose_pattern.find(extracted_string)?;

        println!(
            "Found verbose match at: {}-{}",
            verbose_match.start(),
            verbose_match.end()
        );
        println!("Verbose string: {}", verbose_match.as_str());

        // Calculate absolute positions in the original file
        let absolute_verbose_start = create_element_match.start() + verbose_match.start();
        let absolute_verbose_end = absolute_verbose_start + verbose_match.len();

        Some(LocationResult {
            start_index: absolute_verbose_start,
            end_index: absolute_verbose_end,
            variable_name: None,
        })
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

    /// Find the context low message location in Claude Code's cli.js
    /// Pattern: "Context low (",B,"% remaining) · Run /compact to compact & continue"
    /// where B is a variable name
    pub fn get_context_low_message_location(&self) -> Option<LocationResult> {
        // Pattern to match: "Context low (",{variable},"% remaining) · Run /compact to compact & continue"
        let context_low_pattern = Regex::new(
            r#""Context low \(",([^,]+),"% remaining\) · Run /compact to compact & continue""#,
        )
        .ok()?;

        let context_low_match = context_low_pattern.find(&self.file_content)?;

        println!(
            "Found context low match at: {}-{}",
            context_low_match.start(),
            context_low_match.end()
        );
        println!("Context low string: {}", context_low_match.as_str());

        // Extract the variable name from the capture group
        let captures = context_low_pattern.captures(&self.file_content)?;
        let variable_name = captures.get(1)?.as_str();

        println!("Variable name: {}", variable_name);

        Some(LocationResult {
            start_index: context_low_match.start(),
            end_index: context_low_match.end(),
            variable_name: Some(variable_name.to_string()),
        })
    }

    /// Core robust function locator using anchor-based expansion
    /// Uses stable text patterns to survive Claude Code version updates
    pub fn find_context_low_function_robust(&self) -> Option<LocationResult> {
        // Step 1: Locate stable anchor text that survives obfuscation
        let primary_anchor = "Context low (";
        let anchor_pos = self.file_content.find(primary_anchor)?;

        // Step 2: Search backward within reasonable range to find function declarations
        let search_range = 800; // Optimized range based on actual function size (~466 chars)
        let search_start = anchor_pos.saturating_sub(search_range);
        let backward_text = &self.file_content[search_start..anchor_pos];

        // Find the function declaration that contains our anchor
        let mut function_candidates = Vec::new();
        let mut start = 0;

        while let Some(func_pos) = backward_text[start..].find("function ") {
            let absolute_func_pos = search_start + start + func_pos;

            // Check if this function contains the expected stable patterns
            let func_to_anchor_text = &self.file_content[absolute_func_pos..anchor_pos + 100];

            if func_to_anchor_text.contains("tokenUsage:") {
                function_candidates.push(absolute_func_pos);
                println!("Found function candidate at: {}", absolute_func_pos);
            }

            start += func_pos + 9; // Move past "function "
        }

        // Use the closest function to anchor (last candidate found)
        if let Some(&func_start) = function_candidates.last() {
            println!("Selected function start at: {}", func_start);

            // We only need the function start for condition replacement
            // Return a minimal range that includes the condition
            let condition_search_end = anchor_pos + 100; // Small range after anchor

            Some(LocationResult {
                start_index: func_start,
                end_index: condition_search_end,
                variable_name: Some("context_function".to_string()),
            })
        } else {
            println!("❌ No suitable function candidate found");
            None
        }
    }

    /// Core robust condition locator that finds the if statement to patch
    /// Returns the exact location of 'if(!Q||D)return null' for replacement with 'if(true)return null'
    pub fn get_context_low_condition_location_robust(&self) -> Option<LocationResult> {
        // Find the function using stable patterns
        let function_location = self.find_context_low_function_robust()?;
        let function_content =
            &self.file_content[function_location.start_index..function_location.end_index];

        // Look for if condition pattern using regex - match any condition that returns null
        let if_pattern = Regex::new(r"if\([^)]+\)return null").ok()?;

        if let Some(if_match) = if_pattern.find(function_content) {
            let absolute_start = function_location.start_index + if_match.start();
            let absolute_end = function_location.start_index + if_match.end();

            println!("Found if condition: '{}'", if_match.as_str());

            Some(LocationResult {
                start_index: absolute_start,
                end_index: absolute_end,
                variable_name: Some(if_match.as_str().to_string()),
            })
        } else {
            println!("❌ Could not find if condition in context function");
            None
        }
    }

    /// Disable context low warnings by modifying the if condition to always return null
    /// Uses robust pattern matching based on stable identifiers
    pub fn disable_context_low_warnings(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(location) = self.get_context_low_condition_location_robust() {
            let replacement_condition = "if(true)return null";

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
            Err("Could not locate context low condition using robust method".into())
        }
    }

    /// Write a replacement for the context low message
    pub fn write_context_low_message(
        &mut self,
        new_message: &str,
        variable_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let location = self
            .get_context_low_message_location()
            .ok_or("Failed to find context low message location")?;

        let new_code = format!(
            r#""{}","{}","{}""#,
            new_message.split(',').nth(0).unwrap_or(new_message),
            variable_name,
            new_message.split(',').nth(1).unwrap_or("")
        );

        let new_content = format!(
            "{}{}{}",
            &self.file_content[..location.start_index],
            new_code,
            &self.file_content[location.end_index..]
        );

        self.show_diff(
            "Context Low Message",
            &new_code,
            location.start_index,
            location.end_index,
        );
        self.file_content = new_content;

        Ok(())
    }

    /// Find the ternary condition for esc/interrupt display (new pattern)
    /// Pattern: ="esc",VAR="interrupt"...${...} to ${...}...,...CONDITION?[
    /// Returns the position of CONDITION that needs to be replaced with (false)
    fn find_esc_interrupt_condition_new(&self) -> Option<LocationResult> {
        // Anchor pattern: ="esc" followed by ="interrupt" (variable assignment)
        // Example: SA="esc",_A="interrupt"
        let anchor_pattern = Regex::new(r#"="esc",\w+="interrupt""#).ok()?;

        if let Some(anchor_match) = anchor_pattern.find(&self.file_content) {
            let anchor_pos = anchor_match.start();
            println!(
                "Found esc/interrupt anchor: '{}' at {}",
                anchor_match.as_str(),
                anchor_pos
            );

            // Search forward for the spread ternary pattern: ...VARNAME?[
            let search_range = 800;
            let search_end = (anchor_pos + search_range).min(self.file_content.len());
            let forward_text = &self.file_content[anchor_pos..search_end];

            // Pattern: ...VARNAME?[ where VARNAME is a short identifier
            let spread_pattern = Regex::new(r"\.\.\.(\w+)\?\[").ok()?;

            if let Some(spread_match) = spread_pattern.find(forward_text) {
                let captures = spread_pattern.captures(forward_text)?;
                let var_name = captures.get(1)?;

                // Calculate absolute position of the variable name
                let absolute_start = anchor_pos + spread_match.start() + 3; // Skip "..."
                let absolute_end = absolute_start + var_name.as_str().len();

                println!(
                    "  Found spread ternary: '{}' at {}-{}",
                    var_name.as_str(),
                    absolute_start,
                    absolute_end
                );

                return Some(LocationResult {
                    start_index: absolute_start,
                    end_index: absolute_end,
                    variable_name: Some(var_name.as_str().to_string()),
                });
            } else {
                println!("  ❌ Could not find spread ternary pattern after anchor");
            }
        }

        None
    }

    /// Find the ternary condition for esc/interrupt display (legacy pattern)
    /// Pattern: ...CONDITION?[...{key:"esc"}...,"to interrupt"...]:[]
    /// Returns the position of CONDITION that needs to be replaced with (false)
    fn find_esc_interrupt_condition_legacy(&self) -> Option<LocationResult> {
        let anchor1 = r#"{key:"esc"}"#;
        let anchor2 = r#""to interrupt""#;

        let mut search_start = 0;
        while let Some(anchor1_offset) = self.file_content[search_start..].find(anchor1) {
            let anchor1_pos = search_start + anchor1_offset;

            let search_window_end = (anchor1_pos + 200).min(self.file_content.len());
            let window = &self.file_content[anchor1_pos..search_window_end];

            if window.contains(anchor2) {
                println!(
                    "Found both anchors: {{key:\"esc\"}} at {} and \"to interrupt\" nearby",
                    anchor1_pos
                );

                let before_anchor = &self.file_content[..anchor1_pos];
                if let Some(spread_offset) = before_anchor.rfind("...") {
                    let spread_pos = spread_offset;
                    println!("  Found spread operator at: {}", spread_pos);

                    let between_spread_and_anchor = &self.file_content[spread_pos..anchor1_pos];
                    if let Some(question_offset) = between_spread_and_anchor.find('?') {
                        let question_pos = spread_pos + question_offset;

                        let condition_start = spread_pos + 3;
                        let condition_end = question_pos;

                        let condition = &self.file_content[condition_start..condition_end];
                        println!(
                            "  Found condition '{}' at {}-{}",
                            condition.trim(),
                            condition_start,
                            condition_end
                        );

                        return Some(LocationResult {
                            start_index: condition_start,
                            end_index: condition_end,
                            variable_name: Some(condition.trim().to_string()),
                        });
                    }
                }
            }

            search_start = anchor1_pos + 1;
        }

        None
    }

    /// Find the ternary condition for esc/interrupt display
    /// Tries new pattern first, falls back to legacy pattern
    fn find_esc_interrupt_condition(&self) -> Option<LocationResult> {
        // Try new pattern first
        if let Some(result) = self.find_esc_interrupt_condition_new() {
            return Some(result);
        }

        // Fall back to legacy pattern
        println!("New pattern not found, trying legacy pattern...");
        self.find_esc_interrupt_condition_legacy()
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

    /// Find the Claude in Chrome subscription check location
    /// Uses stable anchors: "tengu_claude_in_chrome_setup" and ".chrome"
    /// Pattern: let VAR=FUNC(PARAM.chrome)&&FUNC2();
    /// Note: Variable/function names change with versions, but ".chrome" and the anchor string are stable
    /// Returns the location of "&&FUNC()" to be removed
    fn find_chrome_subscription_check(&self) -> Option<LocationResult> {
        // Step 1: Find stable anchor that indicates Chrome setup code
        let anchor = "tengu_claude_in_chrome_setup";
        let anchor_pos = self.file_content.find(anchor)?;

        println!("Found anchor '{}' at position: {}", anchor, anchor_pos);

        // Step 2: Search backward to find ".chrome"
        let search_range = 300;
        let search_start = anchor_pos.saturating_sub(search_range);
        let backward_text = &self.file_content[search_start..anchor_pos];

        // Step 3: Find the pattern with .chrome as stable anchor
        // Pattern: let VAR=FUNC(PARAM.chrome)&&FUNC2();
        // We match: FUNC(PARAM.chrome)&&FUNC2() and want to remove &&FUNC2()
        let pattern = Regex::new(r"let\s*\w+=\w+\(\w+\.chrome\)(&&\w+\(\))").ok()?;

        if let Some(captures) = pattern.captures(backward_text) {
            let full_match = captures.get(0)?;
            let and_part = captures.get(1)?; // Captures "&&FUNC2()"

            println!("Found Chrome check pattern: '{}'", full_match.as_str());
            println!("Part to remove: '{}'", and_part.as_str());

            // Calculate absolute position of "&&FUNC2()"
            let match_start_in_backward = full_match.start();
            let and_offset_in_match = and_part.start() - full_match.start();

            let absolute_start = search_start + match_start_in_backward + and_offset_in_match;
            let absolute_end = absolute_start + and_part.as_str().len();

            println!(
                "Found '{}' at position: {}-{}",
                and_part.as_str(),
                absolute_start,
                absolute_end
            );

            return Some(LocationResult {
                start_index: absolute_start,
                end_index: absolute_end,
                variable_name: Some(and_part.as_str().to_string()),
            });
        }

        println!("❌ Could not find Chrome subscription check pattern");
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

    /// Find the /chrome command subscription message location
    /// Pattern: !G&&...createElement(...,"Claude in Chrome requires a claude.ai subscription.")
    /// Returns the location of "!G&&" to be replaced with "false&&"
    fn find_chrome_command_message(&self) -> Option<LocationResult> {
        // Stable anchor: the subscription message string
        let anchor = r#""Claude in Chrome requires a claude.ai subscription.""#;
        let anchor_pos = self.file_content.find(anchor)?;

        println!(
            "Found /chrome subscription message at position: {}",
            anchor_pos
        );

        // Search backward for "!G&&" pattern (or similar variable name)
        let search_range = 100;
        let search_start = anchor_pos.saturating_sub(search_range);
        let backward_text = &self.file_content[search_start..anchor_pos];

        // Pattern: !VARNAME&& where VARNAME is typically a single letter
        let pattern = Regex::new(r"!(\w+)&&").ok()?;

        // Find the last occurrence (closest to anchor)
        let mut last_match: Option<(usize, &str)> = None;
        for mat in pattern.find_iter(backward_text) {
            if let Some(captures) = pattern.captures(mat.as_str()) {
                if let Some(var) = captures.get(1) {
                    last_match = Some((mat.start(), var.as_str()));
                }
            }
        }

        if let Some((offset, var_name)) = last_match {
            let absolute_start = search_start + offset;
            let absolute_end = absolute_start + format!("!{}&&", var_name).len();

            println!(
                "  Found condition '!{}&&' at {}-{}",
                var_name, absolute_start, absolute_end
            );

            return Some(LocationResult {
                start_index: absolute_start,
                end_index: absolute_end,
                variable_name: Some(format!("!{}&&", var_name)),
            });
        }

        println!("  ❌ Could not find !VAR&& pattern before message");
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

    /// Find the Chrome startup notification subscription check
    /// Pattern: if(!zB()){A({key:"chrome-requires-subscription"...
    /// Returns the location of "!zB()" to be replaced with "false"
    fn find_chrome_startup_notification_check(&self) -> Option<LocationResult> {
        // Stable anchor: the unique key for this notification
        let anchor = r#"key:"chrome-requires-subscription""#;
        let anchor_pos = self.file_content.find(anchor)?;

        println!(
            "Found Chrome startup notification anchor at position: {}",
            anchor_pos
        );

        // Search backward for "if(!zB())" or similar pattern
        let search_range = 150;
        let search_start = anchor_pos.saturating_sub(search_range);
        let backward_text = &self.file_content[search_start..anchor_pos];

        // Pattern: if(!FUNC()){  where FUNC is typically 2-3 chars
        // We want to capture the "!FUNC()" part
        let pattern = Regex::new(r"if\((!\w+\(\))\)\{").ok()?;

        // Find the last occurrence (closest to anchor)
        let mut last_match: Option<(usize, String)> = None;
        for cap in pattern.captures_iter(backward_text) {
            if let Some(condition) = cap.get(1) {
                last_match = Some((cap.get(0).unwrap().start(), condition.as_str().to_string()));
            }
        }

        if let Some((match_offset, condition)) = last_match {
            // Calculate position of the condition part (inside the if)
            let if_start = search_start + match_offset;
            let condition_start = if_start + "if(".len();
            let condition_end = condition_start + condition.len();

            println!(
                "  Found condition '{}' at {}-{}",
                condition, condition_start, condition_end
            );

            return Some(LocationResult {
                start_index: condition_start,
                end_index: condition_end,
                variable_name: Some(condition),
            });
        }

        println!("  ❌ Could not find if(!FUNC()) pattern before notification");
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
