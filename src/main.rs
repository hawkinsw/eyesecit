use chrono::{prelude::*, Duration};
use clap::Parser;
use core::time;
use regex::Regex;
use std::thread;

mod backends;
mod bsky_backend;
mod cli;
mod edgar;

use crate::backends::{Backend, BackendConfig, Item105Config, Item105Error};
use crate::bsky_backend::BskyBackendConfig;
use crate::edgar::{
    acceptance_datetime_from_extensions, cik_from_extensions, extract_filings_metadata, json_parse,
    parse_rss, synchronous_download, Filing,
};

//static RSS_URL: &str = "https://www.sec.gov/Archives/edgar/usgaap.rss.xml";
static RSS_URL: &str = "https://www.sec.gov/Archives/edgar/xbrlrss.all.xml";
static JSON_URL: &str = "https://data.sec.gov/submissions/CIK__CIK__.json";

#[tokio::main]
async fn main() -> Result<(), Item105Error> {
    let args = cli::Args::parse();

    let config =
        match Item105Config::config_from_file(args.config.open().unwrap().get_file().unwrap()) {
            Ok(x) => x,
            Err(e) => {
                println!("Error: {e}");
                return Ok(());
            }
        };

    let backends = [BskyBackendConfig {}];

    let mut backend = backends
        .iter()
        .find(|backend| config.backend == backend.name())
        .ok_or(Item105Error {
            msg: format!("Could not find backend for {}", config.backend),
        })
        .and_then(|backend| backend.configure(config.config.to_string()))?;

    // TODO: Make this command-line customizable.
    let period = Duration::minutes(10);
    let mut latest: Option<DateTime<FixedOffset>> = args.start_date;

    print!("Checking for new 8-k entries",);

    if let Some(latest) = latest {
        print!(" since {}.", latest);
    }
    println!(".");

    if let Some(error) = backend.login().await.err() {
        println!("Could not login: {error}");
        return Ok(());
    }

    if args.hello {
        let hello_result = backend.post("Hello!".to_string()).await;
        if let Some(error) = hello_result.err() {
            println!("I failed to post a hello message as requested: {error}");
            return Ok(());
        } else {
            println!("I successfully tweeted a hello message as requested.");
        }
    }

    // Safe here because we confirmed during config-file parse above.
    let alert: Regex = config.alert.parse().unwrap();

    loop {
        let mut processed = 0;
        let now = Local::now().fixed_offset();

        let mut new_latest: Option<DateTime<FixedOffset>> = None;

        println!("It is {:?} ... checking for new entries!", now);

        // Only update the bio if we are not on a dry run!
        if !args.dry {
            let bio_content = format!("{} Last update: {:?}", "This is my bio", now);
            let update_bio_result = backend.status(bio_content).await;
            if update_bio_result.is_ok() {
                println!("I updated my bio.");
            } else {
                println!(
                    "There was an error when I tried to update my bio: {:?}",
                    update_bio_result
                );
            }
        }

        match synchronous_download(RSS_URL).await.and_then(parse_rss) {
            Err(err) => {
                println!("{}", err)
            }
            Ok(feed) => {
                for entry in feed.items {
                    let updated = entry
                        .pub_date()
                        .and_then(|v| {
                            DateTime::parse_from_rfc2822(v)
                                .map_or(Some(Local::now().fixed_offset()), Some)
                        })
                        .unwrap();

                    if let Some(new_latest_date) = new_latest {
                        if updated > new_latest_date {
                            if args.debug > 0 {
                                println!("Marking new latest as {:?}", new_latest_date);
                            }
                            new_latest = Some(updated)
                        }
                    } else {
                        if args.debug > 0 {
                            println!("This is the first item that we are seeing -- setting the baseline new latest to {:?}", updated);
                        }
                        new_latest = Some(updated)
                    }

                    let title = entry.title().unwrap_or("No Title");

                    let acceptance_datetime =
                        acceptance_datetime_from_extensions(&entry.extensions);
                    if acceptance_datetime.is_none() {
                        println!(
                            "Could not gather the acceptance date/time from RSS entry with title {} ... skipping.", title);
                        continue;
                    }
                    let acceptance_datetime = acceptance_datetime.unwrap();

                    let filing_link = entry.link();
                    let cik = cik_from_extensions(entry.extensions());
                    if cik.is_none() {
                        println!(
                            "Could not gather the cik from RSS entry with title {} ... skipping.",
                            title
                        );
                        continue;
                    }
                    let cik = cik.unwrap();

                    if latest.is_none() || updated <= latest.unwrap() {
                        if args.debug > 0 {
                            println!(
                                "Skipping update from {} from {} that we should have seen before.",
                                title, updated
                            );
                        }
                        continue;
                    }

                    if args.debug > 0 {
                        println!("Processing update from {} from {} ...", title, updated);
                    }

                    processed += 1;

                    // TODO: Seems clunky.
                    let formatted_cik = format!("{:0>10}", cik);
                    let json_url = JSON_URL.to_string().replace("__CIK__", &formatted_cik);

                    let filing_download_result: Result<Vec<Filing>, Box<dyn std::error::Error>> = synchronous_download(&json_url)
                    .await.map_err(|err| {
                        String::into(format!(
                            "There was an error downloading the JSON data for company with CIK of {}: {}",
                            cik, err
                        ))
                    })
                    .and_then(|json_string| json_parse(json_string)
                    .map_err(|err| {
                        String::into(format!(
                            "There was an error parsing the JSON data for company with CIK of {}: {}",
                            cik, err
                        ))
                    })
                    .and_then(|recent_form| extract_filings_metadata(recent_form)
                    .map_err(|err| {
                        String::into(format!(
                            "There was an error finding the specifics of the filing from company with CIK of {}: {}",
                            cik, err
                        ))
                    })));

                    match filing_download_result {
                        Ok(filings) => {
                            let filings: Vec<Filing> = filings
                                .into_iter()
                                .filter(|filing| {
                                    if args.debug > 2 {
                                        println!(
                                            "Comparing filing date/time of {} with accepted date/time of {}",
                                            filing.time,
                                            acceptance_datetime
                                        )
                                    }
                                    filing.time == acceptance_datetime
                                })
                                .collect();
                            if args.debug > 0 {
                                println!(
                                    "Found {} new, valid filing(s) posted by {} (cik: {})",
                                    filings.len(),
                                    title,
                                    cik
                                );
                            }
                            for filing in filings {
                                if filing.form == "\"8-K\"" || filing.form == "\"8-K/A\"" {
                                    if args.debug > 0 {
                                        println!(
                                            "{} posted a(n) {} with items {}",
                                            title,
                                            filing.form.clone(),
                                            filing.items.clone()
                                        );
                                    }
                                    if alert.is_match(&filing.items) {
                                        println!(
                                        "{} (cik: {}) filed an 8-K update with an Item that matched the search criteria ({}).",
                                        title, cik, alert);

                                        // If we are on a dry run, then skip the remaining steps!
                                        if args.dry {
                                            continue;
                                        }

                                        let filing_msg = if let Some(filing_link) = filing_link {
                                            format!(" See the filing at {filing_link}")
                                        } else {
                                            "".to_string()
                                        };
                                        let message = format!(
                                            "{} (cik: {}) filed an 8-K update with an Item {}.{}",
                                            title, cik, alert, filing_msg
                                        );
                                        match backend.post(message.clone()).await {
                                            Ok(_) => println!("I posted: {}", message),
                                            Err(e) => println!(
                                                "There was an error when I tried to tweet: {e:?}"
                                            ),
                                        }
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            println!("There was an error downloading the filings: {err}")
                        }
                    }
                    // TODO: Make this command-line customizable.
                    thread::sleep(time::Duration::from_secs(1));
                }
            }
        };

        if args.debug > 0 {
            println!("Updating latest from {:?} to {:?} ...", latest, new_latest);
        }
        latest = new_latest;

        println!("Processed {} entries.", processed);
        println!(
            "Scheduled to process entries again in {:?} minutes.",
            period.num_minutes()
        );
        thread::sleep(period.to_std().unwrap());
    }
}
