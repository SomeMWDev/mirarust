use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use std::error::Error;
use std::fs;
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;

fn get_cloudflare_token() -> String {
    fs::read_to_string("tokens/cloudflare_ssl.txt").expect("Failed to find CF ssl token")
}

fn get_cloudflare_zone_id() -> String {
    fs::read_to_string("tokens/cloudflare_zone.txt").expect("Failed to find CF zone id")
}

fn create_cloudflare_client() -> Result<Client, Box<dyn Error>> {
    let token = get_cloudflare_token();
    let mut headers = HeaderMap::new();
    let auth = format!("Bearer {}", token);
    headers.insert("Authorization", HeaderValue::from_str(auth.as_str())?);
    let client = Client::builder().default_headers(headers).build()?;
    Ok(client)
}

pub fn remove_domain_from_cloudflare(domain: &String) -> Result<(), Box<dyn Error>> {
    let client = create_cloudflare_client()?;
    let url = format!(
        "https://api.cloudflare.com/client/v4/zones/{}/custom_hostnames/{}",
        get_cloudflare_zone_id(),
        domain
    );
    let response = client.delete(url).send()?;
    response.error_for_status()?;
    Ok(())
}

#[derive(Debug, PartialEq)]
pub enum CloudflareDomainStatus {
    Active,
    PendingValidation,
    Expired,
    Other(String),
}

impl FromStr for CloudflareDomainStatus {
    type Err = ();

    fn from_str(input: &str) -> Result<CloudflareDomainStatus, ()> {
        match input {
            "active" => Ok(CloudflareDomainStatus::Active),
            "pending_validation" => Ok(CloudflareDomainStatus::PendingValidation),
            "expired" => Ok(CloudflareDomainStatus::Expired),
            o => Ok(CloudflareDomainStatus::Other(String::from(o))),
        }
    }
}

pub fn cloudflare_fetch() -> Result<Vec<(String, CloudflareDomainStatus)>, Box<dyn Error>> {
    let client = create_cloudflare_client()?;
    let mut page = 1;
    let mut result = Vec::new();
    loop {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/custom_hostnames?per_page=50&page={}",
            get_cloudflare_zone_id(),
            page
        );
        let resp = client.get(url).send()?;
        let json: Value = serde_json::from_str(resp.text()?.as_str())?;
        let response_array = json["result"].as_array().unwrap();
        for domain_data in response_array {
            let name = String::from(domain_data["hostname"].as_str().unwrap());
            let status = String::from(domain_data["ssl"]["status"].as_str().unwrap());
            result.push((
                name,
                CloudflareDomainStatus::from_str(status.as_str()).unwrap(),
            ));
        }
        page += 1;
        if response_array.len() == 0 {
            break;
        }
        sleep(Duration::from_millis(100));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration() {
        cloudflare_fetch().expect("TODO: panic message");
    }
}
