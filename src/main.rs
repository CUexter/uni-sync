use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use notify::{Watcher,  RecursiveMode, Result as NotifyResult};
use std::sync::mpsc::channel;

mod devices;
pub(crate) mod fancurve;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 && args[1] == "--list-sensors" {
        println!("Available sensors:");
        for sensor in fancurve::list_available_sensors() {
            println!("  {}", sensor);
        }
        return Ok(());
    }

    let config_path = if args.len() > 2 && args[1] == "--config" {
        PathBuf::from(&args[2])
    } else {
        PathBuf::from("/etc/uni-sync/uni-sync.json")
    };

    let config_dir = config_path.parent().unwrap_or(Path::new("/etc/uni-sync"));
    let mut configs = load_config(&config_path)?;

    println!("Uni-sync service started.");

    // Set up signal handling
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    // Set up file watching
    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(move |res: NotifyResult<notify::Event>| {
        match res {
            Ok(event) => {
                println!("File change detected: {:?}", event);
                let _ = tx.send(());
            },
            Err(e) => println!("Watch error: {:?}", e),
        }
    })?;

    // Watch the entire config directory recursively
    watcher.watch(config_dir, RecursiveMode::Recursive)?;

    while running.load(Ordering::SeqCst) {
        // Check for file changes
        if rx.try_recv().is_ok() {
            println!("Configuration or fan curve file changed, reloading...");
            configs = load_config(&config_path)?;
        }

        // Run the fan control logic
        configs = devices::run(configs);

        // Wait for a short period before the next iteration
        std::thread::sleep(Duration::from_secs(5));
    }

    println!("Uni-sync service stopped.");
    Ok(())
}

fn load_config(config_path: &Path) -> Result<devices::Configs, Box<dyn std::error::Error>> {
    if config_path.exists() {
        let config_content = std::fs::read_to_string(config_path)?;
        Ok(serde_json::from_str(&config_content)?)
    } else {
        Ok(devices::Configs { configs: vec![] })
    }
}
