use chrono::prelude::*;
use chrono::Duration;
use imap::types::Seq;
use lettre::smtp::authentication::{Credentials, Mechanism};
use lettre::smtp::extension::ClientId;
use lettre::smtp::ConnectionReuseParameters;
use lettre::{EmailTransport, SmtpTransport};
use lettre_email::EmailBuilder;
use mailparse::*;
use rusqlite::{Connection, NO_PARAMS};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs::File;
use std::io::prelude::*;

#[derive(Debug, Deserialize)]
struct Config {
    target_email: String,
    target_name: String,
    db_filename: String,
    journal_email_smtp: String,
    journal_email_imap: String,
    journal_email: String,
    journal_email_password: String,
    utc_reminder_hour: i64,
}

struct JournalEntry {
    _id: i32,
    _day: i32,
    _month: i32,
    _year: i32,
    body: String,
}

struct Email {
    from: String,
    _subject: String,
    timestamp: DateTime<Utc>,
    body: String,
}

impl Email {
    pub fn from_bytes(bytes: &[u8]) -> Email {
        let parsed = parse_mail(bytes).expect("Failed to parse email!");

        let from = parsed.headers.get_first_value("From").unwrap().unwrap();
        let subject = parsed.headers.get_first_value("Subject").unwrap().unwrap();

        let timestamp_rfc2882 = parsed.headers.get_first_value("Date").unwrap().unwrap();
        let timestamp: DateTime<Utc> = DateTime::parse_from_rfc2822(&timestamp_rfc2882)
            .expect("Failed to parse email timestamp!")
            .with_timezone(&Utc);

        let body = {
            if parsed.subparts.len() > 0 {
                parsed.subparts[0].get_body().unwrap()
            } else {
                String::new()
            }
        };

        Email {
            from,
            _subject: subject,
            timestamp,
            body,
        }
    }
}

pub const CONFIG_PATH: &'static str = "config.ron";
pub const SLEEP_TIME_SECONDS: i64 = 2;

// TODO
// - Create default config if one isn't found
// - docs

fn main() {
    // Load config file
    let mut file = File::open(CONFIG_PATH).unwrap();
    let mut config_str = String::new();
    file.read_to_string(&mut config_str).unwrap();

    // Deserialize config
    let config: Config = match ron::de::from_str(&config_str) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    // Validate reminder_hour
    if config.utc_reminder_hour < 0 || config.utc_reminder_hour > 23 {
        eprintln!("Config error! reminder_hour must be an integer between 0 and 23 (inclusive).");
        return;
    }

    initialize_db(&config);

    let utc: DateTime<Utc> = Utc::now();
    let today: DateTime<Utc> = Utc.ymd(utc.year(), utc.month(), utc.day()).and_hms(0, 0, 0);

    let mut did_remind = false;
    let mut remind_time = today
        .checked_add_signed(Duration::hours(config.utc_reminder_hour))
        .unwrap();

    if utc < remind_time {
        println!("Journal reminder for today is scheduled at {}", remind_time);
    } else {
        did_remind = true;
        println!("Journal reminder for today has been sent.");
    }

    println!("Mail Journal running.");

    let sleep_duration = Duration::milliseconds(SLEEP_TIME_SECONDS).to_std().unwrap();
    loop {
        let utc: DateTime<Utc> = Utc::now();

        // Check for new journal emails
        let seqs = search_inbox_latest(&config).expect("Failed to search for latest emails!");

        // Check for new journal emails
        if seqs.len() > 0 {
            println!("{} new email(s)", seqs.len());

            let emails = fetch_emails(&config, seqs).expect("Failed to fetch emails!");

            for email in emails {
                store_journal_email(&config, &email);
            }
        }

        // Handle journal reminder
        if !did_remind && (utc >= remind_time) {
            // Remind the user again in exactly 1 day
            remind_time = remind_time.checked_add_signed(Duration::days(1)).unwrap();

            did_remind = true;
            send_reminder_email(&config);

            println!(
                "Journal reminder for {} sent. Next reminder scheduled for {}",
                utc.to_string(),
                remind_time.to_string()
            );
        }

        std::thread::sleep(sleep_duration);
    }
}

fn initialize_db(config: &Config) {
    let sql_conn = Connection::open(&config.db_filename).expect("Failed to open database!");

    sql_conn
        .execute(
            "CREATE TABLE IF NOT EXISTS entries (
                  id    INTEGER PRIMARY KEY,
                  day   INTEGER NOT NULL,
                  month INTEGER NOT NULL,
                  year  INTEGER NOT NULL,
                  body  TEXT NOT NULL
                  )",
            NO_PARAMS,
        )
        .unwrap();
}

fn send_reminder_email(config: &Config) {
    let mut message =
        String::from("How was your day today? Reply to this email with your daily journal entry.");

    // Fetch past journal entries on this day and add them to the message
    let entries = fetch_past_journal_entries(&config);
    if entries.len() > 0 {
        message.push_str("\n\nOn this day, one year ago:\n");
        for entry in entries {
            message.push_str(&format!("\"{}\"", entry.body.trim()));
        }
    }

    let email = EmailBuilder::new()
        .to((config.target_email.clone(), config.target_name.clone()))
        .from((config.journal_email.clone(), "Mail Journal"))
        .subject("Daily Journal Entry")
        .text(message)
        .build()
        .unwrap();

    let mut mailer = SmtpTransport::simple_builder(&config.journal_email_smtp)
        .unwrap()
        .hello_name(ClientId::Domain(config.journal_email_smtp.clone()))
        .credentials(Credentials::new(
            config.journal_email.clone(),
            config.journal_email_password.clone(),
        ))
        // Enable SMTPUTF8 if the server supports it
        .smtp_utf8(true)
        // Configure expected authentication mechanism
        .authentication_mechanism(Mechanism::Plain)
        // Enable connection reuse
        .connection_reuse(ConnectionReuseParameters::ReuseUnlimited)
        .build();

    let result = mailer.send(&email);
    assert!(result.is_ok());

    // Explicitly close the SMTP transaction as we enabled connection reuse
    mailer.close();
}

fn send_error_email(config: &Config, msg: &str) {
    let email = EmailBuilder::new()
        .to((config.target_email.clone(), config.target_name.clone()))
        .from((config.journal_email.clone(), "Mail Journal"))
        .subject("Error")
        .text(msg)
        .build()
        .unwrap();

    let mut mailer = SmtpTransport::simple_builder(&config.journal_email_smtp)
        .unwrap()
        .hello_name(ClientId::Domain(config.journal_email_smtp.clone()))
        .credentials(Credentials::new(
            config.journal_email.clone(),
            config.journal_email_password.clone(),
        ))
        // Enable SMTPUTF8 if the server supports it
        .smtp_utf8(true)
        // Configure expected authentication mechanism
        .authentication_mechanism(Mechanism::Plain)
        // Enable connection reuse
        .connection_reuse(ConnectionReuseParameters::ReuseUnlimited)
        .build();

    let result = mailer.send(&email);
    assert!(result.is_ok());

    // Explicitly close the SMTP transaction as we enabled connection reuse
    mailer.close();
}

fn store_journal_email(config: &Config, email: &Email) {
    if (email.from != config.target_email)
        && (!email.from.contains(&format!("<{}>", config.target_email)))
    {
        println!("Ignoring email from {}", email.from);
        return;
    }

    let day = &email.timestamp.day().to_string();
    let month = &email.timestamp.month().to_string();
    let year = &email.timestamp.year().to_string();

    let sql_conn = Connection::open(&config.db_filename).expect("Failed to open database!");

    // We need to check if there is already an entry for this day
    let stmt_str = format!(
        "SELECT day, month, year FROM entries WHERE day = {} AND month = {} AND year = {}",
        email.timestamp.day(),
        email.timestamp.month(),
        email.timestamp.year()
    );

    let mut stmt = sql_conn.prepare(&stmt_str).unwrap();
    if stmt.exists(NO_PARAMS).unwrap() {
        println!("Journal entry for today was already submitted, ignoring new entry.");
        send_error_email(config, "You already submitted a journal entry for today!");

        return;
    }

    // Store the entry
    sql_conn
        .execute(
            "INSERT INTO entries (day, month, year, body) values (?1, ?2, ?3, ?4)",
            &[&day, &month, &year, &email.body],
        )
        .unwrap();
}

fn fetch_past_journal_entries(config: &Config) -> Vec<JournalEntry> {
    let sql_conn = Connection::open(&config.db_filename).expect("Failed to open database!");

    let date = Utc::now();
    let date = Utc
        .ymd(date.year(), date.month(), date.day())
        .checked_sub_signed(Duration::days(365))
        .unwrap();

    let query_str = format!(
        "SELECT id, day, month, year, body FROM entries WHERE month = {} AND day = {} AND year = {}",
        date.month(), date.day(), date.year()
    );

    let mut stmt = sql_conn.prepare(&query_str).unwrap();

    let entry_iter = stmt
        .query_map(NO_PARAMS, |row| JournalEntry {
            _id: row.get(0),
            _day: row.get(1),
            _month: row.get(2),
            _year: row.get(3),
            body: row.get(4),
        })
        .unwrap()
        .map(|s| s.unwrap());

    return entry_iter.collect::<Vec<JournalEntry>>();
}

fn fetch_emails(config: &Config, seqs: HashSet<Seq>) -> imap::error::Result<Vec<Email>> {
    let domain = config.journal_email_imap.as_str();
    let tls = native_tls::TlsConnector::builder().build().unwrap();

    // Connect to the email server and login
    let client = imap::connect((domain, 993), domain, &tls).unwrap();
    let mut imap_session = client
        .login(&config.journal_email, &config.journal_email_password)
        .map_err(|e| e.0)?;

    imap_session.select("INBOX")?;

    // Construct the sequence string, which is just
    // the email sequence numbers separated by spaces
    let mut seq_str = String::new();
    for seq in seqs {
        seq_str.push_str(&format!("{},", seq));
    }

    // Trim the extra whitespace and comma off the sequence string
    seq_str = seq_str.trim_end().trim_end_matches(',').to_string();

    // Fetch emails
    let mut emails: Vec<Email> = Vec::new();

    println!("Fetching emails from sequence: {}", seq_str);
    let fetched = imap_session.fetch(seq_str, "RFC822")?;
    for m in fetched.iter() {
        emails.push(Email::from_bytes(m.body().unwrap()));
    }

    imap_session.logout()?;

    Ok(emails)
}

fn search_inbox_latest(config: &Config) -> imap::error::Result<HashSet<imap::types::Seq>> {
    let domain = config.journal_email_imap.as_str();
    let tls = native_tls::TlsConnector::builder().build().unwrap();

    // Connect to the email server and login
    let client = imap::connect((domain, 993), domain, &tls).unwrap();
    let mut imap_session = client
        .login(&config.journal_email, &config.journal_email_password)
        .map_err(|e| e.0)?;

    imap_session.select("INBOX")?;

    let query = format!(
        "UNSEEN FROM {} SINCE {}",
        &config.target_email,
        Utc::now().format("%d-%b-%Y").to_string()
    );
    let seqs = imap_session.search(query)?;

    imap_session.logout()?;

    Ok(seqs)
}
