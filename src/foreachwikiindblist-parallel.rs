use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
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

    /// The amount of threads to use per cluster at the same time
    #[arg(long, short)]
    concurrent_cluster_threads: Option<usize>,

    #[arg(trailing_var_arg = true)]
    script: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let script = cli.script;
    if script.is_empty() {
        bail!("Please provide a script to run!")
    }

    let concurrent_threads = cli.concurrent_cluster_threads
        .unwrap_or(1);

    let output = Command::new("/usr/bin/php")
        .args(["-r", PHP_CODE])
        .arg(&cli.db_list)
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
        .flat_map(|(cluster, dbs)| {
            println!("Cluster {cluster} has {} dbs.", dbs.len());
            let dbs_arc = Arc::new(Mutex::new(dbs));
            let mut threads: Vec<JoinHandle<()>> = vec![];

            for i in 0..concurrent_threads {
                let cmd = script.clone();
                let cluster = cluster.clone();
                let dbs_arc = Arc::clone(&dbs_arc);

                threads.push(thread::spawn(move || {
                    println!("Starting thread {i} on {cluster}");
                    loop {
                        let db = {
                            let mut dbs = dbs_arc.lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            dbs.pop()
                        };

                        let Some(db) = db else {
                            println!("Ending thread {i} on {cluster}");
                            break;
                        };

                        let out = Command::new(&cmd[0])
                            .args(&cmd[1..])
                            .args(["--wiki", &db])
                            .output();
                        match out {
                            Err(e) => println!("{cluster}/{i} Error on {db}: {:?}", e),
                            Ok(output) => {
                                println!("{cluster}/{i} Script completed on {db} ({cluster})");

                                let stdout_c = String::from_utf8_lossy(&output.stdout);
                                let stderr_c = String::from_utf8_lossy(&output.stderr);
                                let stdout = stdout_c.trim();
                                let stderr = stderr_c.trim();

                                if !stdout.is_empty() {
                                    println!("{cluster}/{i} OUT: {stdout}");
                                }
                                if !stderr.is_empty() {
                                    eprintln!("{cluster}/{i} ERR: {stderr}");
                                }
                            }
                        }
                    }
                }))
            }
            threads
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
    Ok(())
}
