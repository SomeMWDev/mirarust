use crate::cloudflare_checker::{CloudflareDomainStatus, cloudflare_fetch};
use crate::custom_domain_checker::{DomainStatus, check_domain_status};
use crate::domain_db::{
    domain_expired, get_connection, get_domains_past_threshold, remove_domains_from_table,
    remove_good_domains_from_table,
};
mod cloudflare_checker;
mod custom_domain_checker;
mod domain_db;
mod remove_domains;

fn process_problematic_domains(domains: Vec<String>) {
    let conn = get_connection("problematic_domains".to_string());
    let epoch = rand::random::<i64>();
    domains.iter().for_each(|domain| {
        domain_expired(&conn, domain, epoch);
    });
    remove_good_domains_from_table(&conn, epoch);
    let need_removal = get_domains_past_threshold(&conn, 3);
    let successful_removals = remove_domains::remove_domains(&need_removal);
    remove_domains_from_table(&conn, successful_removals);
}

fn main() {
    let mut cloudflare_results = cloudflare_fetch().expect("Failed to fetch Cloudflare SSL data");
    let _ = cloudflare_results.extract_if(.., |(_, status)| -> bool {
        match status {
            CloudflareDomainStatus::Other(message) => {
                println!("Unknown Cloudflare status {}", message);
                true
            }
            _ => false,
        }
    });
    let (working_domains, problematic_domains): (Vec<_>, Vec<_>) = cloudflare_results
        .iter()
        .partition(|(_, status)| -> bool { *status != CloudflareDomainStatus::Expired });
    if problematic_domains.len() > cloudflare_results.len() / 10 {
        panic!("More than 10% of Cloudflare domains are problematic. This can't be right.");
    }
    println!(
        "{} domains total. {} problematic ones.",
        cloudflare_results.len(),
        problematic_domains.len()
    );
    let client = custom_domain_checker::ReqwestClient::new().expect("Failed to create HTTP client");
    process_problematic_domains(
        problematic_domains
            .into_iter()
            .map(|(s, _)| -> String { s.to_string() })
            .filter(|s| match check_domain_status(&client, s.as_str()) {
                DomainStatus::Ok => {
                    println!(
                        "Cloudflare reported an error for {} but it seems to be fine.",
                        s
                    );
                    false
                }
                _ => true,
            })
            .collect(),
    );
}
