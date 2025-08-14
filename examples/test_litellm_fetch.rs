use serde::Deserialize;
use std::collections::HashMap;

// Test struct matching our current implementation (with Option fields)
#[derive(Debug, Clone, Deserialize)]
pub struct LiteLLMPricing {
    pub input_cost_per_token: Option<f64>,
    pub output_cost_per_token: Option<f64>,
    #[serde(default)]
    pub cache_creation_input_token_cost: Option<f64>,
    #[serde(default)]
    pub cache_read_input_token_cost: Option<f64>,
}

// Test with extra fields allowed
#[derive(Debug, Clone, Deserialize)]
pub struct LiteLLMPricingFlexible {
    pub input_cost_per_token: Option<f64>,
    pub output_cost_per_token: Option<f64>,
    #[serde(default)]
    pub cache_creation_input_token_cost: Option<f64>,
    #[serde(default)]
    pub cache_read_input_token_cost: Option<f64>,
    // Catch all extra fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[tokio::main]
async fn main() {
    println!("Testing LiteLLM Pricing Fetch");
    println!("==============================\n");

    // Test 1: Fetch the actual data
    println!("1. Fetching from LiteLLM...");
    let response = match reqwest::get("https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json").await {
        Ok(r) => r,
        Err(e) => {
            println!("   ✗ Failed to fetch: {}", e);
            return;
        }
    };
    println!("   ✓ Successfully fetched data");

    // Test 2: Get response text
    let text = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            println!("   ✗ Failed to get text: {}", e);
            return;
        }
    };
    println!("   ✓ Got response text ({} bytes)", text.len());

    // Test 3: Parse as generic JSON first
    println!("\n2. Parsing as generic JSON...");
    let json_value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            println!("   ✗ Failed to parse as JSON: {}", e);
            println!("   First 500 chars: {}", &text[..500.min(text.len())]);
            return;
        }
    };
    println!("   ✓ Successfully parsed as JSON");

    // Test 4: Check structure
    if let Some(obj) = json_value.as_object() {
        println!("   Found {} top-level keys", obj.len());

        // Find Claude models
        let claude_models: Vec<_> = obj.keys().filter(|k| k.contains("claude")).collect();
        println!("   Found {} Claude models", claude_models.len());

        // Show first Claude model
        if let Some(model_name) = claude_models.first() {
            println!("\n3. Examining model: {}", model_name);
            if let Some(model_data) = obj.get(*model_name) {
                // Try to parse this specific model
                match serde_json::from_value::<LiteLLMPricing>(model_data.clone()) {
                    Ok(pricing) => {
                        println!("   ✓ Successfully parsed with strict struct:");
                        println!("     Input cost: {:?}", pricing.input_cost_per_token);
                        println!("     Output cost: {:?}", pricing.output_cost_per_token);
                    }
                    Err(e) => {
                        println!("   ✗ Failed with strict struct: {}", e);
                    }
                }

                // Try with flexible struct
                match serde_json::from_value::<LiteLLMPricingFlexible>(model_data.clone()) {
                    Ok(pricing) => {
                        println!("   ✓ Successfully parsed with flexible struct:");
                        println!("     Input cost: {:?}", pricing.input_cost_per_token);
                        println!("     Output cost: {:?}", pricing.output_cost_per_token);
                        println!("     Extra fields: {}", pricing.extra.len());
                    }
                    Err(e) => {
                        println!("   ✗ Failed with flexible struct: {}", e);
                    }
                }
            }
        }
    }

    // Test 5: Try parsing full HashMap
    println!("\n4. Parsing as HashMap<String, LiteLLMPricing>...");
    match serde_json::from_str::<HashMap<String, LiteLLMPricing>>(&text) {
        Ok(data) => {
            println!("   ✓ Successfully parsed!");
            let claude_count = data.keys().filter(|k| k.contains("claude")).count();
            println!("   Found {} Claude models in HashMap", claude_count);
        }
        Err(e) => {
            println!("   ✗ Failed to parse: {}", e);

            // Try to identify which field causes the issue
            if e.to_string().contains("unknown field") {
                println!("   Issue: Unknown field in response");
            } else if e.to_string().contains("invalid type") {
                println!("   Issue: Type mismatch");
            }
        }
    }

    // Test 6: Try with flexible struct
    println!("\n5. Parsing as HashMap<String, LiteLLMPricingFlexible>...");
    match serde_json::from_str::<HashMap<String, LiteLLMPricingFlexible>>(&text) {
        Ok(data) => {
            println!("   ✓ Successfully parsed with flexible struct!");
            let claude_count = data.keys().filter(|k| k.contains("claude")).count();
            println!("   Found {} Claude models", claude_count);

            // Show some pricing data for Claude models with valid pricing
            let mut shown = 0;
            for (name, pricing) in data.iter() {
                if name.contains("claude")
                    && pricing.input_cost_per_token.is_some()
                    && pricing.output_cost_per_token.is_some()
                    && shown < 3
                {
                    println!("\n   Model: {}", name);
                    println!(
                        "   - Input: ${:.6}/token",
                        pricing.input_cost_per_token.unwrap()
                    );
                    println!(
                        "   - Output: ${:.6}/token",
                        pricing.output_cost_per_token.unwrap()
                    );
                    shown += 1;
                }
            }
        }
        Err(e) => {
            println!("   ✗ Failed with flexible struct: {}", e);
        }
    }
}
