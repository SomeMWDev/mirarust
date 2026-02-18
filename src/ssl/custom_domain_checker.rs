use reqwest::StatusCode;
use reqwest::header::{HeaderMap, HeaderValue};

pub trait HttpClient {
    fn get(&self, url: &str) -> Result<(StatusCode, String), reqwest::Error>;
}

pub struct ReqwestClient {
    client: reqwest::blocking::Client,
}

impl ReqwestClient {
    pub fn new() -> Result<Self, reqwest::Error> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "user-agent",
            HeaderValue::from_static("Miraheze custom domain scanner bot"),
        );
        let client = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .build()?;
        Ok(ReqwestClient { client })
    }
}

impl HttpClient for ReqwestClient {
    fn get(&self, url: &str) -> Result<(StatusCode, String), reqwest::Error> {
        let resp = self.client.get(url).send()?;
        let status_code = resp.status();
        let text = resp.text()?;
        Ok((status_code, text))
    }
}

#[derive(Debug, PartialEq)]
pub enum DomainStatus {
    // All is good
    Ok,
    WikiNotFound,
    DomainMisconfigured,
    Expired,
    // MediaWiki site not hosted on Miraheze
    MediaWikiWebsite,
    // Random website not using MediaWiki
    Website,
    // An unknown error occurred
    Unknown(String),
}

pub fn check_domain_status(client: &dyn HttpClient, url: &str) -> DomainStatus {
    match client.get(url) {
        Ok((status_code, text)) => check_page_content(status_code, text.as_str()),
        Err(e) => classify_error(e),
    }
}

fn classify_error(err: reqwest::Error) -> DomainStatus {
    let error_string = format!("{:?}", err);

    if error_string.contains("dns error") {
        DomainStatus::Expired
    } else {
        DomainStatus::Unknown(error_string)
    }
}

fn check_page_content(_status_code: StatusCode, response: &str) -> DomainStatus {
    use regex::Regex;
    let body_check = Regex::new("<body class=[^>]{0,1000}mediawiki").unwrap();
    // This is a MediaWiki site
    if body_check.is_match(response) {
        if response.contains("footer-mirahezeico") || response.contains("meta.miraheze.org") {
            return DomainStatus::Ok;
        } else {
            return DomainStatus::MediaWikiWebsite;
        }
    }
    // Not a MediaWiki site. Now determine what it is.
    if response.contains("<title>Wiki not found</title>") {
        return DomainStatus::WikiNotFound;
    }
    let lower = response.to_ascii_lowercase();
    if lower.contains("domain misconfigured") {
        return DomainStatus::DomainMisconfigured;
    }
    if lower.contains("expired") {
        return DomainStatus::Expired;
    }
    DomainStatus::Website
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_url_status(url: &str, expected: DomainStatus) {
        let client = ReqwestClient::new().expect("Failed to create HTTP client");
        assert_eq!(check_domain_status(&client, url), expected);
    }

    #[test]
    fn test_integration() {
        assert_url_status("https://en.wikipedia.org", DomainStatus::MediaWikiWebsite);
        assert_url_status("https://strinova.org", DomainStatus::Ok);
        assert_url_status("https://atlas.starworld.zone", DomainStatus::Expired);
    }
}
