use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use anyhow::{bail, Context};
use clap::{Parser};


const PHP_CODE: &str = r#"$config = include $argv[1];
$clusters = [];
foreach ( $config['databases'] as $dbName => $dbInfo ) {
	$clusters[$dbInfo['c']][] = $dbName;
}
echo json_encode($clusters);"#;

#[derive(Parser)]
#[command(version)]
struct Cli {
    db_list: PathBuf,

    #[arg(trailing_var_arg = true)]
    script: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let script = cli.script;
    if script.is_empty() {
        bail!("Please provide a script to run!")
    }

    let output = Command::new("/usr/bin/php")
        .args(["-r", PHP_CODE, cli.db_list.as_os_str().to_str().unwrap()])
        .output()
        .context("Failed to run PHP command")?;
    if !output.status.success() {
        bail!(
            "PHP failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    }

    let json = String::from_utf8(output.stdout)?;
    let clusters: HashMap<String, Vec<String>> = serde_json::from_str(json.as_str())
        .with_context(|| format!("Invalid JSON: {json}"))?;
    println!("Found databases on {} clusters.", &clusters.len());

    let handles: Vec<_> = clusters
        .into_iter()
        .map(|(cluster, dbs)| {
            println!("{} has {} dbs.", cluster, dbs.len());
            let cmd = script.clone();
            thread::spawn(move || {
                for db in dbs {
                    let out = Command::new(&cmd[0])
                        .args(&cmd[1..])
                        .args(["--wiki", &db])
                        .output();
                    match out {
                        Err(e) => println!("Error on {db}: {:?}", e),
                        Ok(output) => {
                            println!("Script completed on {db} ({cluster})");
                            let stdout_c = String::from_utf8_lossy(&output.stdout);
                            let stderr_c = String::from_utf8_lossy(&output.stderr);
                            let stdout = stdout_c.trim();
                            let stderr = stderr_c.trim();
                            if !stdout.is_empty() {
                                println!("OUT: {stdout}");
                            }
                            if !stderr.is_empty() {
                                eprintln!("ERR: {stderr}");
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
    Ok(())
}
