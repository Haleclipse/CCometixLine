use ccometixline::config::Config;

fn main() {
    match Config::load() {
        Ok(config) => {
            println!("Loaded {} segments:", config.segments.len());
            for (i, s) in config.segments.iter().enumerate() {
                println!("  {}. {:?} (enabled: {})", i + 1, s.id, s.enabled);
            }
        }
        Err(e) => {
            eprintln!("Error loading config: {}", e);
        }
    }
}
