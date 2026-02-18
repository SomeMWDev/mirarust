use sqlite::{Connection, State};

pub fn get_connection(filename: String) -> Connection {
    let connection = Connection::open(filename).expect("Failed to open database");

    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS domains (
                url TEXT PRIMARY KEY,
                fails INTEGER NOT NULL DEFAULT 0,
                epoch INTEGER NOT NULL
            )",
        )
        .expect("Failed to create table");

    connection
}

pub fn domain_expired(connection: &Connection, url: &String, epoch: i64) {
    let mut statement = connection
        .prepare("SELECT fails FROM domains WHERE url = ?1")
        .expect("Failed to prepare select statement");

    statement
        .bind((1, url.as_str()))
        .expect("Failed to bind url");

    let new_fails: i64 = if let State::Row = statement.next().expect("Failed to execute select") {
        let fails: i64 = statement.read(0).expect("Failed to read column fails");
        fails + 1
    } else {
        1
    };
    drop(statement);

    let mut statement = connection
        .prepare("INSERT OR REPLACE INTO domains (url, fails, epoch) VALUES (?1, ?2, ?3)")
        .expect("Failed to prepare insert statement");

    statement
        .bind((1, url.as_str()))
        .expect("Failed to bind url");
    statement
        .bind((2, new_fails))
        .expect("Failed to bind fails");
    statement.bind((3, epoch)).expect("Failed to bind epoch");
    statement
        .next()
        .expect(format!("Failed to execute insert on {}", url).as_str());
}

pub fn get_domains_past_threshold(connection: &Connection, fails: i32) -> Vec<String> {
    let mut statement = connection
        .prepare("SELECT url FROM domains WHERE fails >= ?1")
        .expect("Failed to prepare select statement");

    statement
        .bind((1, fails as i64))
        .expect("Failed to bind column fails");

    let mut domains = Vec::new();
    while let State::Row = statement.next().expect("Failed to execute select") {
        let url: String = statement.read(0).expect("Failed to read url");
        domains.push(url);
    }

    domains
}

pub fn remove_good_domains_from_table(connection: &Connection, epoch: i64) {
    let mut statement = connection
        .prepare("DELETE FROM domains WHERE epoch != ?1")
        .expect("Failed to prepare delete statement");

    statement.bind((1, epoch)).expect("Failed to bind epoch");
    statement.next().expect("Failed to execute delete");
}

pub fn remove_domains_from_table(connection: &Connection, domains: Vec<String>) {
    for domain in domains {
        let mut statement = connection
            .prepare("DELETE FROM domains WHERE url = ?1")
            .expect("Failed to prepare delete statement");

        statement
            .bind((1, domain.as_str()))
            .expect("Failed to bind url");
        statement.next().expect("Failed to execute delete");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let filename = "test.sqlite".to_string();
        get_connection(filename)
    }

    fn cleanup_test_db() {
        std::fs::remove_file("test.sqlite").ok();
    }

    #[test]
    fn test_complete_workflow() {
        let conn = setup_test_db();
        let url1 = "https://example.com".to_string();
        let url2 = "https://frequen-fai.com".to_string();
        let epoch1 = 1000;
        let epoch2 = 2000;

        domain_expired(&conn, &url1, epoch1);
        domain_expired(&conn, &url2, epoch1);
        domain_expired(&conn, &url2, epoch1);

        let domains_1_fail = get_domains_past_threshold(&conn, 1);
        assert_eq!(domains_1_fail.len(), 2);
        assert!(domains_1_fail.contains(&url1));
        assert!(domains_1_fail.contains(&url2));

        let domains_3_fails = get_domains_past_threshold(&conn, 2);
        assert_eq!(domains_3_fails.len(), 1);
        assert_eq!(domains_3_fails[0], url2);

        domain_expired(&conn, &url2, epoch2);

        remove_good_domains_from_table(&conn, epoch2);

        assert_eq!(get_domains_past_threshold(&conn, 1).len(), 1);
        assert_eq!(get_domains_past_threshold(&conn, 3).len(), 1);

        remove_domains_from_table(&conn, vec![url2]);
        let final_domains = get_domains_past_threshold(&conn, 0);
        assert_eq!(final_domains.len(), 0);

        cleanup_test_db();
    }
}
