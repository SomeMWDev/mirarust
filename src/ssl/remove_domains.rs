use crate::cloudflare_checker::remove_domain_from_cloudflare;
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::error::Error;
use std::process::Command;

fn run_maintenance_script(db_name: &String) -> Result<()> {
    let output = Command::new("getMWVersion")
        .arg(db_name)
        .output()
        .context("Failed to run getMWVersion command")?;
    if !output.status.success() {
        bail!(
            "getMWVersion failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    }
    let version = String::from_utf8(output.stdout)?.trim().to_string();

    let output = Command::new("sudo")
        .args(["-u", "www-data", "php"])
        .arg(format!("/srv/mediawiki/{}/maintenance/run.php", version))
        .args(["MirahezeMagic:RemoveCustomDomain", "--wiki", db_name])
        .output()
        .context("Failed to run maintenance script to remove custom domain")?;
    if !output.status.success() {
        bail!(
            "MediaWiki maintenance script failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    }
    Ok(())
}

fn get_domain_to_db_name_mapping() -> Result<HashMap<String, String>, Box<dyn Error>> {
    let url =
        "https://raw.githubusercontent.com/miraheze/ssl/refs/heads/main/wikidiscover_output.yaml";
    let client = Client::new();

    let response = client.get(url).send()?;

    let text = response.text()?;

    let mut mapping = HashMap::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 {
            continue;
        }

        let db_name = parts[0].trim();
        let url_part = parts[1].trim();

        let url_match = if let Some(stripped) = url_part.strip_prefix("https://") {
            stripped
        } else {
            continue;
        };

        let domain = url_match.trim_end_matches('/').to_string();
        mapping.insert(domain, db_name.to_string());
    }

    Ok(mapping)
}

pub fn remove_domains(domains: &Vec<String>) -> Vec<String> {
    let domain_to_db_name =
        get_domain_to_db_name_mapping().expect("Cannot fetch domain name to db name mapping.");
    domains
        .iter()
        .filter(|domain| -> bool {
            let res = remove_domain_from_cloudflare(domain);
            if res.is_err() {
                println!("{:?}", res);
                return false;
            }
            let res = domain_to_db_name.get(*domain);
            if res.is_none() {
                println!("No db name found for domain {}", domain);
                return false;
            }
            let db_name = res.unwrap();
            println!("I will now remove {} from db name {}", domain, db_name);
            let res2 = run_maintenance_script(domain);
            if res2.is_err() {
                println!("{:?}", res);
                return false;
            }
            true
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strinova_org_mapping() {
        let mapping = get_domain_to_db_name_mapping().expect("Failed to fetch mapping");
        assert_eq!(
            mapping.get("strinova.org"),
            Some(&"strinovawiki".to_string())
        );
    }
}
