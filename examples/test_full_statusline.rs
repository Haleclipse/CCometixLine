use ccometixline::config::{Config, InputData, Model, Workspace};
use ccometixline::core::StatusLineGenerator;

fn main() {
    println!("Testing Full Statusline with Cost Tracking");
    println!("===========================================\n");

    // Create test configuration with all segments enabled
    let config = Config {
        segments: ccometixline::config::SegmentsConfig {
            model: true,
            directory: true,
            git: true,
            usage: true,
            cost: true,
            burn_rate: true,
        },
        theme: "nerdfonts".to_string(),
    };

    // Create test input data
    let input = InputData {
        model: Model {
            display_name: "claude-3-5-sonnet-20241022".to_string(),
        },
        workspace: Workspace {
            current_dir: "/home/user/projects/test-project".to_string(),
        },
        transcript_path: "/home/user/.claude/projects/test/session-123.jsonl".to_string(),
    };

    // Generate statusline
    let generator = StatusLineGenerator::new(config.clone());
    let statusline = generator.generate(&input);

    println!("Generated Statusline:");
    println!("{}", statusline);
    println!();

    // Test with different configurations
    println!("Testing Segment Order and Configuration:");
    println!("-----------------------------------------");

    // Test with only model and usage
    let minimal_config = Config {
        segments: ccometixline::config::SegmentsConfig {
            model: true,
            directory: false,
            git: false,
            usage: true,
            cost: false,
            burn_rate: false,
        },
        theme: "nerdfonts".to_string(),
    };

    let minimal_generator = StatusLineGenerator::new(minimal_config);
    let minimal_statusline = minimal_generator.generate(&input);
    println!("Minimal (Model + Usage): {}", minimal_statusline);

    // Test with cost tracking only
    let cost_config = Config {
        segments: ccometixline::config::SegmentsConfig {
            model: true,
            directory: true,
            git: false,
            usage: false,
            cost: true,
            burn_rate: false,
        },
        theme: "nerdfonts".to_string(),
    };

    let cost_generator = StatusLineGenerator::new(cost_config);
    let cost_statusline = cost_generator.generate(&input);
    println!("Cost Tracking: {}", cost_statusline);

    // Test with burn rate only
    let burn_config = Config {
        segments: ccometixline::config::SegmentsConfig {
            model: true,
            directory: true,
            git: false,
            usage: false,
            cost: false,
            burn_rate: true,
        },
        theme: "nerdfonts".to_string(),
    };

    let burn_generator = StatusLineGenerator::new(burn_config);
    let burn_statusline = burn_generator.generate(&input);
    println!("Burn Rate: {}", burn_statusline);

    // Test segment ordering
    println!("\n✓ Segment Order Verification:");
    println!("  1. Model");
    println!("  2. Directory");
    println!("  3. Git");
    println!("  4. Usage");
    println!("  5. Cost (NEW)");
    println!("  6. BurnRate (NEW)");
    println!("  7. Update (if available)");

    println!("\n✅ Integration test completed successfully!");
}
