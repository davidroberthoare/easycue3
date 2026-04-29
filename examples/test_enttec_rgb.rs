//! Test program for Enttec DMXUSB Pro with 6-channel RGBAWUV LED control
//!
//! Usage: cargo run --example test_enttec_rgb --features usb
//!
//! This example:
//! - Lists available serial ports
//! - Connects to an Enttec DMXUSB Pro device
//! - Controls channels 1-6 as R, G, B, A(mber), W(hite), UV
//! - Provides interactive control to test your LED fixture

use easycue3::dmx::{Universe, backends::{DmxBackend, EnttecUsbProBackend}};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    println!("=== Enttec DMXUSB Pro 6-Channel RGBAWUV Test ===\n");
    
    // List available serial ports
    #[cfg(feature = "usb")]
    {
        println!("Scanning for serial ports...");
        let ports = EnttecUsbProBackend::list_ports()?;
        
        if ports.is_empty() {
            eprintln!("No serial ports found!");
            eprintln!("Make sure your Enttec DMXUSB Pro is plugged in.");
            return Ok(());
        }
        
        println!("Available serial ports:");
        for (i, port) in ports.iter().enumerate() {
            println!("  [{}] {}", i, port);
        }
        println!();
        
        // Prompt user to select port
        print!("Select port number (or press Enter for port 0): ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let port_index: usize = input.trim().parse().unwrap_or(0);
        
        if port_index >= ports.len() {
            eprintln!("Invalid port selection!");
            return Ok(());
        }
        
        let selected_port = &ports[port_index];
        println!("\nConnecting to {}...", selected_port);
        
        // Create backend
        let mut backend = EnttecUsbProBackend::new(selected_port)?;
        println!("✓ Connected to Enttec DMXUSB Pro");
        println!("Backend: {}\n", backend.name());
        
        // Create a universe
        let mut universe = Universe::new(0);
        
        // Main control loop
        println!("6-Channel RGBAWUV LED Control");
        println!("  Channel 1 = Red");
        println!("  Channel 2 = Green");
        println!("  Channel 3 = Blue");
        println!("  Channel 4 = Amber");
        println!("  Channel 5 = White");
        println!("  Channel 6 = UV");
        println!();
        println!("Commands:");
        println!("  r <0-100>      - Set Red (channel 1)");
        println!("  g <0-100>      - Set Green (channel 2)");
        println!("  b <0-100>      - Set Blue (channel 3)");
        println!("  a <0-100>      - Set Amber (channel 4)");
        println!("  w <0-100>      - Set White (channel 5)");
        println!("  uv <0-100>     - Set UV (channel 6)");
        println!("  ch <ch> <val>  - Set any channel directly (1-512)");
        println!("  scan           - Scan channels 1-6 individually");
        println!("  test           - Run color test sequence");
        println!("  off            - Turn off all channels");
        println!("  quit           - Exit program");
        println!();
        
        loop {
            print!("> ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();
            
            if input.is_empty() {
                continue;
            }
            
            let parts: Vec<&str> = input.split_whitespace().collect();
            let command = parts[0].to_lowercase();
            
            match command.as_str() {
                "quit" | "q" | "exit" => {
                    println!("Turning off channels and exiting...");
                    universe.clear();
                    backend.send_universe(&universe)?;
                    break;
                }
                
                "off" => {
                    println!("Turning off all channels");
                    universe.clear();
                    backend.send_universe(&universe)?;
                }
                
                "r" | "red" => {
                    if parts.len() < 2 {
                        println!("Usage: r <0-100>");
                        continue;
                    }
                    match parts[1].parse::<u8>() {
                        Ok(val) if val <= 100 => {
                            universe.set_channel(1, val)?;
                            backend.send_universe(&universe)?;
                            println!("Red (ch1) set to {}", val);
                        }
                        _ => println!("Value must be 0-100"),
                    }
                }
                
                "g" | "green" => {
                    if parts.len() < 2 {
                        println!("Usage: g <0-100>");
                        continue;
                    }
                    match parts[1].parse::<u8>() {
                        Ok(val) if val <= 100 => {
                            universe.set_channel(2, val)?;
                            backend.send_universe(&universe)?;
                            println!("Green (ch2) set to {}", val);
                        }
                        _ => println!("Value must be 0-100"),
                    }
                }
                
                "b" | "blue" => {
                    if parts.len() < 2 {
                        println!("Usage: b <0-100>");
                        continue;
                    }
                    match parts[1].parse::<u8>() {
                        Ok(val) if val <= 100 => {
                            universe.set_channel(3, val)?;
                            backend.send_universe(&universe)?;
                            println!("Blue (ch3) set to {}", val);
                        }
                        _ => println!("Value must be 0-100"),
                    }
                }
                                "a" | "amber" => {
                    if parts.len() < 2 {
                        println!("Usage: a <0-100>");
                        continue;
                    }
                    match parts[1].parse::<u8>() {
                        Ok(val) if val <= 100 => {
                            universe.set_channel(4, val)?;
                            backend.send_universe(&universe)?;
                            println!("Amber (ch4) set to {}", val);
                        }
                        _ => println!("Value must be 0-100"),
                    }
                }
                
                "w" | "white" => {
                    if parts.len() < 2 {
                        println!("Usage: w <0-100>");
                        continue;
                    }
                    match parts[1].parse::<u8>() {
                        Ok(val) if val <= 100 => {
                            universe.set_channel(5, val)?;
                            backend.send_universe(&universe)?;
                            println!("White (ch5) set to {}", val);
                        }
                        _ => println!("Value must be 0-100"),
                    }
                }
                
                "uv" => {
                    if parts.len() < 2 {
                        println!("Usage: uv <0-100>");
                        continue;
                    }
                    match parts[1].parse::<u8>() {
                        Ok(val) if val <= 100 => {
                            universe.set_channel(6, val)?;
                            backend.send_universe(&universe)?;
                            println!("UV (ch6) set to {}", val);
                        }
                        _ => println!("Value must be 0-100"),
                    }
                }
                                "a" | "amber" => {
                    if parts.len() < 2 {
                        println!("Usage: a <0-100>");
                        continue;
                    }
                    match parts[1].parse::<u8>() {
                        Ok(val) if val <= 100 => {
                            universe.set_channel(4, val)?;
                            backend.send_universe(&universe)?;
                            println!("Amber (ch4) set to {}", val);
                        }
                        _ => println!("Value must be 0-100"),
                    }
                }
                
                "w" | "white" => {
                    if parts.len() < 2 {
                        println!("Usage: w <0-100>");
                        continue;
                    }
                    match parts[1].parse::<u8>() {
                        Ok(val) if val <= 100 => {
                            universe.set_channel(5, val)?;
                            backend.send_universe(&universe)?;
                            println!("White (ch5) set to {}", val);
                        }
                        _ => println!("Value must be 0-100"),
                    }
                }
                
                "uv" => {
                    if parts.len() < 2 {
                        println!("Usage: uv <0-100>");
                        continue;
                    }
                    match parts[1].parse::<u8>() {
                        Ok(val) if val <= 100 => {
                            universe.set_channel(6, val)?;
                            backend.send_universe(&universe)?;
                            println!("UV (ch6) set to {}", val);
                        }
                        _ => println!("Value must be 0-100"),
                    }
                }
                
                "ch" | "channel" => {
                    if parts.len() < 3 {
                        println!("Usage: ch <channel> <value>  (channel 1-512, value 0-100)");
                        continue;
                    }
                    match (parts[1].parse::<u16>(), parts[2].parse::<u8>()) {
                        (Ok(ch), Ok(val)) if ch >= 1 && ch <= 512 && val <= 100 => {
                            universe.set_channel(ch, val)?;
                            backend.send_universe(&universe)?;
                            println!("Channel {} set to {}", ch, val);
                        }
                        _ => println!("Invalid: channel must be 1-512, value 0-100"),
                    }
                }
                
                "scan" => {
                    println!("Scanning channels 1-6 (watch which color/effect appears)...");
                    universe.clear();
                    backend.send_universe(&universe)?;
                    thread::sleep(Duration::from_millis(500));
                    
                    let channel_names = ["Red", "Green", "Blue", "Amber", "White", "UV"];
                    for ch in 1u16..=6u16 {
                        println!("  Channel {} ({}) at 100%", ch, channel_names[(ch - 1) as usize]);
                        universe.set_channel(ch, 100)?;
                        backend.send_universe(&universe)?;
                        thread::sleep(Duration::from_millis(1500));
                        universe.set_channel(ch, 0)?;
                        backend.send_universe(&universe)?;
                        thread::sleep(Duration::from_millis(300));
                    }
                    
                    println!("Scan complete!");
                }
                
                "test" => {
                    println!("Running 6-channel color test sequence...");
                    
                    // Format: (R, G, B, A, W, UV, name)
                    let test_colors = [
                        (100, 0, 0, 0, 0, 0, "Red"),
                        (0, 100, 0, 0, 0, 0, "Green"),
                        (0, 0, 100, 0, 0, 0, "Blue"),
                        (0, 0, 0, 100, 0, 0, "Amber"),
                        (0, 0, 0, 0, 100, 0, "White"),
                        (0, 0, 0, 0, 0, 100, "UV"),
                        (100, 100, 0, 0, 0, 0, "Yellow (R+G)"),
                        (0, 100, 100, 0, 0, 0, "Cyan (G+B)"),
                        (100, 0, 100, 0, 0, 0, "Magenta (R+B)"),
                        (100, 100, 100, 0, 0, 0, "White (RGB)"),
                        (50, 50, 50, 50, 50, 0, "Mixed (no UV)"),
                        (0, 0, 0, 0, 0, 0, "Off"),
                    ];
                    
                    for (r, g, b, a, w, uv, name) in test_colors.iter() {
                        println!("  {}", name);
                        universe.set_channel(1, *r)?;
                        universe.set_channel(2, *g)?;
                        universe.set_channel(3, *b)?;
                        universe.set_channel(4, *a)?;
                        universe.set_channel(5, *w)?;
                        universe.set_channel(6, *uv)?;
                        backend.send_universe(&universe)?;
                        thread::sleep(Duration::from_millis(1500));
                    }
                    
                    println!("Test complete!");
                }
                
                _ => {
                    println!("Unknown command: {}", command);
                    println!("Type quit to exit or see commands above");
                }
            }
        }
        
        backend.close()?;
        Ok(())
    }
    
    #[cfg(not(feature = "usb"))]
    {
        eprintln!("USB support not enabled!");
        eprintln!("Build with: cargo run --example test_enttec_rgb --features usb");
        Ok(())
    }
}
